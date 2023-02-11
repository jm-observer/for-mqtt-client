use crate::v3_1_1::*;
use anyhow::Context;
use anyhow::Result;
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
pub use data::*;

#[derive(Clone, Debug)]
enum Command {}

/// duty: 1. tcp connect
///     2. send connect packet
pub struct TaskNetwork {
    addr: String,
    port: u16,
    connect_packet: Arc<Bytes>,
    senders: Senders,
    is_connected: bool,
    rx: mpsc::Receiver<Data>,
    tx: mpsc::Sender<NetworkStaus>,
}

impl TaskNetwork {
    pub fn init(
        addr: String,
        port: u16,
        inner_tx: Senders,
        rx: mpsc::Receiver<Data>,
        connect_packet: Arc<Bytes>,
        tx: mpsc::Sender<NetworkStaus>,
    ) -> Self {
        Self {
            addr,
            port,
            senders: inner_tx,
            rx,
            is_connected: false,
            connect_packet,
            tx,
        }
    }
    pub fn run(mut self) {
        tokio::spawn(async move {
            debug!("{}: {}", self.addr, self.port);
            let mut buf = BytesMut::with_capacity(10 * 1024);
            let mut stream = self.try_connect(&mut buf).await;
            loop {
                if !self.is_connected {
                    buf = BytesMut::with_capacity(10 * 1024);
                    stream = self.try_connect(&mut buf).await;
                }
                select! {
                    read = stream.read_buf(&mut buf) => match read  {
                        Ok(len) => {
                            if len == 0 {
                                error!("tcp read 0 size");
                                self.is_connected = false;
                                continue;
                            }
                            if let Err(e) = self.deal_network_msg(&mut buf).await {
                                error!("{:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("{:?}", e);
                            self.is_connected = false;
                            continue;
                        }

                    },
                    val = self.rx.recv() => match val {
                        Some(val) => {
                            debug!("{:?}", val);
                            self.deal_inner_msg(&mut stream, val).await;
                            continue;
                        }
                        None => {
                            error!("None");
                            continue;
                        }
                    }
                }
            }
        });
    }

    async fn network_disconnect(&mut self, error: String) {
        self.is_connected = false;
        if self.tx.send(NetworkStaus::Disconnect(error)).await.is_err() {
            error!("");
        }
    }

    async fn deal_network_msg(&mut self, buf: &mut BytesMut) -> Result<()> {
        // todo to optimize
        let max_size = 1024;
        loop {
            match self.parse(buf, max_size).await {
                Ok(_) => {
                    // 处理粘包
                    if buf.len() >= 2 {
                        continue;
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                    break;
                }
            }
        }
        Ok(())
    }
    async fn deal_inner_msg(&mut self, stream: &mut TcpStream, msg: Data) {
        match msg {
            Data::NetworkData(val) => {
                let Err(e) = stream.write_all(val.as_ref().as_ref()).await else {
                    val.done();
                    return;
                };
                error!("{:?}", e);
                self.network_disconnect(e.to_string()).await;
            }
            Data::Reconnect => {
                self.is_connected = false;
            }
        }
    }

    async fn try_connect(&mut self, buf: &mut BytesMut) -> TcpStream {
        loop {
            debug!("tcp connect……");
            match TcpStream::connect((self.addr.as_str(), self.port)).await {
                Ok(mut stream) => {
                    info!("tcp connect success");
                    if let Err(e) = stream.write_all(self.connect_packet.as_ref()).await {
                        error!("{:?}", e);
                    } else {
                        loop {
                            match stream.read_buf(buf).await {
                                Ok(len) => {
                                    if len == 0 {
                                        break;
                                    }
                                    if let Err(e) = self.deal_network_msg(buf).await {
                                        error!("{:?}", e);
                                        continue;
                                    }
                                    if self.is_connected {
                                        return stream;
                                    }
                                }
                                Err(e) => {
                                    error!("{:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("{:?}", e);
                }
            };
            sleep(Duration::from_secs(5)).await;
        }
    }

    /// Reads a stream of bytes and extracts next MQTT packet out of it
    async fn parse(&mut self, stream: &mut BytesMut, max_size: usize) -> anyhow::Result<()> {
        let fixed_header = check(stream.iter(), max_size)?;
        let packet = stream.split_to(fixed_header.frame_length());
        let packet_type = fixed_header.packet_type()?;
        debug!("{:?}", packet_type);
        if fixed_header.remaining_len() == 0 {
            // no payload packets
            return match packet_type {
                PacketType::PingReq => {
                    warn!("should not receive pingreq");
                    Ok(())
                }
                PacketType::PingResp => {
                    self.senders
                        .broadcast_tx
                        .tx_ping
                        .send(PingResp)
                        .context("send ping resp fail")?;
                    Ok(())
                }
                PacketType::Disconnect => {
                    warn!("should not receive disconnect");
                    Ok(())
                }
                _ => Err(Error::PayloadRequired)?,
            };
        }

        let packet = packet.freeze();
        match packet_type {
            // PacketType::Connect => Packet::Connect(Connect::read(fixed_header, packet)?),
            PacketType::ConnAck => {
                let _ = ConnAck::read(fixed_header, packet)?;
                self.is_connected = true;
                self.tx.send(NetworkStaus::Connected).await?;
            }
            PacketType::Publish => {
                self.tx_publish(Publish::read(fixed_header, packet)?).await;
            }
            PacketType::PubAck => {
                self.tx_publish_ack(PubAck::read(fixed_header, packet)?)
                    .await;
            }
            PacketType::PubRec => {
                self.tx_publish_rec(PubRec::read(fixed_header, packet)?)
                    .await;
            }
            PacketType::PubRel => {
                self.tx_publish_rel(PubRel::read(fixed_header, packet)?)
                    .await;
            }
            PacketType::PubComp => {
                self.tx_publish_comp(PubComp::read(fixed_header, packet)?)
                    .await;
            }
            // PacketType::Subscribe => Packet::Subscribe(Subscribe::read(fixed_header, packet)?),
            PacketType::SubAck => {
                self.tx_sub_ack(SubAck::read(fixed_header, packet)?.into())
                    .await;
            }
            // PacketType::Unsubscribe => Packet::Unsubscribe(Unsubscribe::read(fixed_header, packet)?),
            PacketType::UnsubAck => {
                self.tx_unsub_ack(UnsubAck::read(fixed_header, packet)?.into())
                    .await;
            }
            // PacketType::PingReq => Packet::PingReq,
            PacketType::PingResp => {
                warn!("PingResp should be zero byte");
                self.senders.broadcast_tx.tx_ping.send(PingResp)?;
            }
            // PacketType::Disconnect => Packet::Disconnect,
            ty => {
                warn!("should not receive {:?}", ty);
            }
        };
        Ok(())
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
            .tx_hub
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
        if self.senders.tx_hub.send(msg).await.is_err() {
            error!("fail to send connector");
        }
    }
}
