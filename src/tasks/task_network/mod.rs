use anyhow::{anyhow, Result};
use anyhow::{bail, Context};
use bytes::{Bytes, BytesMut};
use log::{debug, error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::sleep;

mod data;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{
    read_from_network, Disconnect, Packet, PacketParseError, PingResp, PubAck, PubComp, PubRec,
    PubRel, Publish, SubAck, UnsubAck,
};
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
    rx: mpsc::Receiver<Data>,
    // tx: mpsc::Sender<NetworkEvent>,
    state: NetworkState,
}

/// 一旦断开就不再连接，交由hub去维护后续的连接
impl TaskNetwork {
    pub fn init(
        addr: String,
        port: u16,
        inner_tx: Senders,
        rx: mpsc::Receiver<Data>,
        connect_packet: Bytes,
        // tx: mpsc::Sender<NetworkEvent>,
    ) -> Self {
        Self {
            addr,
            port,
            senders: inner_tx,
            rx,
            state: NetworkState::ToConnect,
            connect_packet,
            // tx,
        }
    }
    pub fn run(mut self) {
        tokio::spawn(async move {
            if let Err(e) = self._run().await {
                error!("{:?}", e);
                let _ = self
                    .senders
                    .tx_hub_network_event
                    .send(NetworkEvent::Disconnect(e.to_string()))
                    .await;
            }
        });
    }
    async fn _run(&mut self) -> Result<()> {
        debug!("{}: {}", self.addr, self.port);
        let mut buf = BytesMut::with_capacity(10 * 1024);
        let mut stream = self.run_to_connect(&mut buf).await?;
        loop {
            if self.state.is_to_connect() {
                debug!("run_to_connect……");
                stream = self.run_to_connect(&mut buf).await?;
            } else if self.state.is_connected() {
                debug!("tcp connect……");
                self.run_connected(&mut stream, &mut buf).await?;
            } else if self.state.is_to_disconnected() {
                self.run_to_disconnected(&mut stream).await?;
            } else {
                break;
            }
        }
        Ok(())
    }
    async fn run_connected(
        &mut self,
        stream: &mut TcpStream,
        buf: &mut BytesMut,
    ) -> Result<(), Error> {
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                read_len = stream.read_buf(buf) => {
                    if read_len? == 0 {
                        return Err(Error::NetworkError("read 0 byte from network".to_string()));
                    }
                    self.deal_connected_network_packet(buf).await?;
                },
                val = self.rx.recv() => {
                    self.deal_inner_msg(stream, val.ok_or(Error::RecvDataFail)?).await?;
                }
            }
        }
    }
    async fn run_to_disconnected(&mut self, stream: &mut TcpStream) -> Result<()> {
        stream.write_all(Disconnect::data()).await?;
        self.senders
            .tx_hub_network_event
            .send(NetworkEvent::Disconnected)
            .await?;
        self.state = NetworkState::Disconnected;
        Ok(())
    }

    async fn deal_connected_network_packet(&mut self, buf: &mut BytesMut) -> Result<(), Error> {
        loop {
            match read_from_network(buf) {
                Ok(packet) => {
                    match packet {
                        Packet::ConnAck(packet) => {
                            warn!("Unexpected ConnAck");
                            return Ok(());
                        }
                        Packet::Publish(packet) => {
                            self.tx_publish(packet).await;
                        }
                        Packet::PubAck(packet) => {
                            self.tx_publish_ack(packet).await;
                        }
                        Packet::PubRec(packet) => {
                            self.tx_publish_rec(packet).await;
                        }
                        Packet::PubRel(packet) => {
                            self.tx_publish_rel(packet).await;
                        }
                        Packet::PubComp(packet) => {
                            self.tx_publish_comp(packet).await;
                        }
                        Packet::SubAck(packet) => {
                            self.tx_sub_ack(packet).await;
                        }
                        Packet::UnsubAck(packet) => {
                            self.tx_unsub_ack(packet).await;
                        }
                        Packet::PingResp => {
                            self.senders.broadcast_tx.tx_ping.send(PingResp)?;
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
    async fn deal_inner_msg(&mut self, stream: &mut TcpStream, mut msg: Data) -> Result<(), Error> {
        debug!("{:?}", msg);

        // todo 考虑发布粘包
        let mut other_msg: bool = false;
        let mut to_send_datas = Vec::new();
        match msg {
            Data::NetworkData(val) => {
                to_send_datas.push(val);
            }
            Data::Disconnect => {
                self.state = NetworkState::ToDisconnect;
                return Ok(());
            }
        }
        while let Ok(data) = self.rx.try_recv() {
            match data {
                Data::NetworkData(val) => {
                    to_send_datas.push(val);
                }
                Data::Disconnect => {
                    other_msg = true;
                    break;
                }
            }
        }
        for data in to_send_datas.iter() {
            stream.write_all(data.data.as_ref()).await?;
        }
        for data in to_send_datas {
            data.done();
        }
        if other_msg {
            self.state = NetworkState::ToDisconnect;
        }
        Ok(())
    }

    async fn run_to_connect(&mut self, buf: &mut BytesMut) -> anyhow::Result<TcpStream> {
        loop {
            match self._run_to_connect(buf).await {
                Ok(stream) => {
                    self.senders
                        .tx_hub_network_event
                        .send(NetworkEvent::Connected)
                        .await?;
                    self.state = NetworkState::Connected;
                    return Ok(stream);
                }
                Err(err) => {
                    error!("{:?}", err);
                }
            }
            sleep(Duration::from_secs(5)).await;
        }
    }
    async fn _run_to_connect(&mut self, buf: &mut BytesMut) -> Result<TcpStream, ToConnectError> {
        let mut stream = TcpStream::connect((self.addr.as_str(), self.port)).await?;
        info!("tcp connect success");
        stream.write_all(self.connect_packet.as_ref()).await?;
        let len = stream.read_buf(buf).await?;
        let packet = read_from_network(buf)?;
        let packet_ty = packet.packet_ty();
        let Packet::ConnAck(ack) = packet else {
            return Err(ToConnectError::NotConnAck(packet_ty));
        };
        if ack.code.is_success() {
            // todo session_present
            Ok(stream)
        } else {
            Err(ToConnectError::BrokerRefuse(ack.code))
        }
    }

    async fn tx_publish_rel(&self, msg: PubRel) {
        if self.senders.broadcast_tx.tx_pub_rel.send(msg).is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_publish_ack(&self, msg: PubAck) {
        if self.senders.broadcast_tx.tx_pub_ack.send(msg).is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_publish_rec(&self, msg: PubRec) {
        if self.senders.broadcast_tx.tx_pub_rec.send(msg).is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_publish_comp(&self, msg: PubComp) {
        if self.senders.broadcast_tx.tx_pub_comp.send(msg).is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_publish(&self, msg: Publish) {
        if self
            .senders
            .tx_hub_msg
            .send(HubMsg::RxPublish(msg))
            .await
            .is_err()
        {
            error!("fail to send publisher");
        }
    }
    async fn tx_sub_ack(&self, msg: SubAck) {
        if self.senders.broadcast_tx.tx_sub_ack.send(msg).is_err() {
            error!("fail to send subscriber");
        }
    }
    async fn tx_unsub_ack(&self, msg: UnsubAck) {
        if self.senders.broadcast_tx.tx_unsub_ack.send(msg).is_err() {
            error!("fail to send subscriber");
        }
    }
    async fn tx_connect_rel(&self, msg: HubMsg) {
        if self.senders.tx_hub_msg.send(msg).await.is_err() {
            error!("fail to send connector");
        }
    }
}
