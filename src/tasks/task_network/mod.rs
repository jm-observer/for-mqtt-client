use anyhow::Result;

use bytes::{Bytes, BytesMut};
use log::{debug, error, warn};

use tokio::select;
use tokio::sync::mpsc;

mod data;

use crate::tasks::task_hub::HubMsg;

use crate::protocol::packet::Disconnect;
use crate::protocol::packet::PingResp;
use crate::protocol::packet::{read_from_network, ConnectReturnCode, Packet};
use crate::protocol::{NetworkProtocol, Protocol};
use crate::tasks::Senders;
pub use data::*;

#[derive(Clone, Debug)]
enum Command {}

/// duty: 1. tcp connect
///     2. send connect packet
pub struct TaskNetwork {
    addr: String,
    port: u16,
    connect_packet: Bytes,
    senders: Senders,
    rx_data: mpsc::Receiver<DataWaitingToBeSend>,
    rx_hub_network_command: mpsc::Receiver<HubNetworkCommand>,
    state: NetworkState,
    version: Protocol,
    network_protocol: NetworkProtocol,
}

/// 一旦断开就不再连接，交由hub去维护后续的连接
impl TaskNetwork {
    pub fn init(
        addr: String,
        port: u16,
        inner_tx: Senders,
        rx: mpsc::Receiver<DataWaitingToBeSend>,
        connect_packet: Bytes,
        rx_hub_network_command: mpsc::Receiver<HubNetworkCommand>,
        version: Protocol,
        network_protocol: NetworkProtocol,
    ) -> Self {
        Self {
            addr,
            port,
            senders: inner_tx,
            rx_data: rx,
            state: NetworkState::ToConnect,
            connect_packet,
            rx_hub_network_command,
            version,
            network_protocol,
        }
    }
    pub fn run(mut self) {
        tokio::spawn(async move {
            if let Err(e) = self._run().await {
                error!("{:?}", e);
                match e {
                    NetworkTasksError::NetworkError(msg) => {
                        let _ = self
                            .senders
                            .tx_hub_network_event
                            .send(NetworkEvent::ConnectedErr(msg))
                            .await;
                    }
                    NetworkTasksError::ConnectFail(reason) => {
                        let _ = self
                            .senders
                            .tx_hub_network_event
                            .send(NetworkEvent::ConnectFail(reason))
                            .await;
                    }
                    NetworkTasksError::ChannelAbnormal => {}
                    NetworkTasksError::HubCommandToDisconnect => {
                        warn!("should not be reached")
                    }
                }
            }
        });
    }
    async fn _run(&mut self) -> Result<(), NetworkTasksError> {
        debug!("{}: {}", self.addr, self.port);
        let mut buf = BytesMut::with_capacity(10 * 1024);
        let mut stream = self.run_to_connect(&mut buf).await?;
        loop {
            if self.state.is_connected() {
                debug!("tcp connect……");
                if let Err(e) = self.run_connected(&mut stream, &mut buf).await {
                    if e == NetworkTasksError::HubCommandToDisconnect {
                        continue;
                    } else {
                        return Err(e);
                    }
                }
            } else if self.state.is_to_disconnected() {
                self.run_to_disconnected(&mut stream).await?;
            } else {
                debug!("{:?}", self.state);
                break;
            }
        }
        Ok(())
    }
    async fn run_connected(
        &mut self,
        stream: &mut Stream,
        buf: &mut BytesMut,
    ) -> Result<(), NetworkTasksError> {
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                read_len = stream.read_buf(buf) => {
                    if read_len? == 0 {
                        return Err(NetworkTasksError::NetworkError("read 0 byte from network".to_string()));
                    }
                    self.deal_connected_network_packet(buf).await?;
                },
                val = self.rx_data.recv() => {
                    self.deal_inner_msg(stream, val.ok_or(NetworkTasksError::ChannelAbnormal)?).await?;
                }
                command = self.rx_hub_network_command.recv() => {
                    self.deal_hub_network_command(command.ok_or(NetworkTasksError::ChannelAbnormal)?)?;
                }
            }
        }
    }
    async fn run_to_disconnected(&mut self, stream: &mut Stream) -> Result<(), NetworkTasksError> {
        stream
            .write_all(Disconnect::new(self.version).data().as_ref())
            .await?;
        self.state = NetworkState::Disconnected;
        Ok(())
    }

    ///
    async fn run_to_connect(&mut self, buf: &mut BytesMut) -> Result<Stream, ToConnectError> {
        let mut stream = Stream::init(self.network_protocol.clone(), &self.addr, self.port).await?;
        // let mut stream = TcpStream::connect((self.addr.as_str(), self.port)).await?;
        let session_present = self._run_to_connect(&mut stream, buf).await?;
        self.senders
            .tx_hub_network_event
            .send(NetworkEvent::Connected(session_present))
            .await?;
        self.state = NetworkState::Connected;
        return Ok(stream.into());
    }

    /// 连接到broker
    async fn _run_to_connect(
        &mut self,
        stream: &mut Stream,
        buf: &mut BytesMut,
    ) -> Result<bool, ToConnectError> {
        stream.write_all(self.connect_packet.as_ref()).await?;
        let _len = stream.read_buf(buf).await?;
        let packet = read_from_network(buf, self.version)?;
        let packet_ty = packet.packet_ty();
        let Packet::ConnAck(ack) = packet else {
            return Err(ToConnectError::NotConnAck(packet_ty));
        };
        match ack.code {
            ConnectReturnCode::Success => Ok(ack.session_present),
            ConnectReturnCode::Fail(code) => Err(ToConnectError::BrokerRefuse(code)),
        }
    }

    async fn deal_connected_network_packet(
        &mut self,
        buf: &mut BytesMut,
    ) -> Result<(), NetworkTasksError> {
        loop {
            match read_from_network(buf, self.version) {
                Ok(packet) => {
                    match packet {
                        Packet::ConnAck(_packet) => {
                            warn!("Unexpected ConnAck");
                        }
                        Packet::Publish(packet) => {
                            if self
                                .senders
                                .tx_hub_msg
                                .send(HubMsg::RxPublish(packet))
                                .await
                                .is_err()
                            {
                                error!("fail to send Publish");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::PubAck(packet) => {
                            if self.senders.broadcast_tx.tx_pub_ack.send(packet).is_err() {
                                error!("fail to send PubAck");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::PubRec(packet) => {
                            if self.senders.broadcast_tx.tx_pub_rec.send(packet).is_err() {
                                error!("fail to send PubRec");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::PubRel(packet) => {
                            if self.senders.broadcast_tx.tx_pub_rel.send(packet).is_err() {
                                error!("fail to send PubRel");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::PubComp(packet) => {
                            if self.senders.broadcast_tx.tx_pub_comp.send(packet).is_err() {
                                error!("fail to send PubComp");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::SubAck(packet) => {
                            if self.senders.broadcast_tx.tx_sub_ack.send(packet).is_err() {
                                error!("fail to send SubAck");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::UnsubAck(packet) => {
                            if self.senders.broadcast_tx.tx_unsub_ack.send(packet).is_err() {
                                error!("fail to send UnsubAck");
                                return Err(NetworkTasksError::ChannelAbnormal);
                            }
                        }
                        Packet::PingResp => {
                            self.senders.broadcast_tx.tx_ping.send(PingResp)?;
                        }
                        Packet::Disconnect(packet) => {
                            self.senders
                                .tx_hub_network_event
                                .send(NetworkEvent::BrokerDisconnect(packet))
                                .await?
                        }
                        // Packet::Connect(_) => {}
                        // Packet::Subscribe(_) => {}
                        // Packet::Unsubscribe(_) => {}
                        packet => {
                            error!("should not be rx: {:?}", packet);
                        }
                    };
                }
                Err(err) => {
                    error!("{:?}", err);
                }
            }
            if buf.len() >= 2 {
                continue;
            } else {
                return Ok(());
            }
        }
    }
    async fn deal_inner_msg(
        &mut self,
        stream: &mut Stream,
        msg: DataWaitingToBeSend,
    ) -> Result<(), NetworkTasksError> {
        let _other_msg: bool = false;
        let mut to_send_datas = vec![msg];
        while let Ok(data) = self.rx_data.try_recv() {
            to_send_datas.push(data);
        }
        // todo packet too big?
        for data in to_send_datas.iter() {
            stream.write_all(data.data.as_ref()).await?;
        }
        for data in to_send_datas {
            data.done();
        }
        Ok(())
    }

    // fn try_deal_hub_network_command(&mut self) -> Result<(), NetworkTasksError> {
    //     loop {
    //         return match self.rx_hub_network_command.try_recv() {
    //             Ok(command) => {
    //                 self.deal_hub_network_command(command)?;
    //                 Ok(())
    //             }
    //             Err(TryRecvError::Disconnected) => Err(NetworkTasksError::ChannelAbnormal),
    //             Err(TryRecvError::Empty) => Ok(()),
    //         };
    //     }
    // }
    fn deal_hub_network_command(
        &mut self,
        command: HubNetworkCommand,
    ) -> Result<(), NetworkTasksError> {
        match command {
            HubNetworkCommand::Disconnect => {
                self.state = NetworkState::ToDisconnect;
                Err(NetworkTasksError::HubCommandToDisconnect)
            }
        }
    }
}
