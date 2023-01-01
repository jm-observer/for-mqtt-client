use crate::v3_1_1::*;
use anyhow::Context;
use bytes::BytesMut;
use log::{debug, error, info, warn};
use std::io;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::mpsc;
use tokio::time::sleep;
use url::Url;

mod data;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::Senders;
pub use data::*;

#[derive(Clone, Debug)]
enum Command {}

pub struct TaskNetwork {
    addr: String,
    port: u16,
    senders: Senders,
    sub_task_tx: Sender<Command>,
    rx: mpsc::Receiver<NetworkData>,
}

impl TaskNetwork {
    pub fn init(
        addr: String,
        port: u16,
        inner_tx: Senders,
        rx: mpsc::Receiver<NetworkData>,
    ) -> Self {
        let (sub_task_tx, _) = channel(1024);
        Self {
            addr,
            port,
            senders: inner_tx,
            sub_task_tx,
            rx,
        }
    }
    pub fn run(mut self) {
        tokio::spawn(async move {
            debug!("{}: {}", self.addr, self.port);
            let stream = self.try_connect().await;
            let (reader, mut writer) = tokio::io::split(stream);
            SubTaskNetworkReader::init(self.senders.clone(), reader, self.sub_task_tx.clone());
            let mut buf = BytesMut::with_capacity(1024);
            loop {
                match self.rx.recv().await {
                    Some(val) => {
                        debug!("{:?}", val);
                        let Err(e) = writer.write_all(val.as_ref().as_ref()).await else {
                            // debug!("done");
                            val.done();
                            continue;
                        };
                        error!("{:?}", e);
                    }
                    None => {
                        error!("None");
                        break;
                    }
                }
            }
        });
    }

    async fn try_connect(&self) -> TcpStream {
        loop {
            let Ok(stream) = TcpStream::connect((self.addr.as_str(), self.port))
                .await else {
                continue;
            };
            info!("tcp connect success");
            return stream;
        }
    }
}

struct SubTaskNetworkReader {
    senders: Senders,
    reader: ReadHalf<TcpStream>,
    sub_task_tx: Sender<Command>,
}

impl SubTaskNetworkReader {
    fn init(senders: Senders, reader: ReadHalf<TcpStream>, sub_task_tx: Sender<Command>) {
        let reader = Self {
            senders,
            reader,
            sub_task_tx,
        };
        tokio::spawn(async move {
            reader.run().await;
        });
    }
    async fn run(mut self) {
        let max_size = 1024;
        let mut buf = BytesMut::with_capacity(10 * 1024);
        // let mut buf = [0u8; 10240];
        loop {
            let read = match self.reader.read_buf(&mut buf).await {
                Ok(len) => len,
                Err(e) => {
                    // 测试
                    error!("{:?}", e);
                    sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };
            if 0 == read {
                warn!("");
                // continue;
                break;
            }
            match self.parse(&mut buf, max_size).await {
                Ok(packet) => {}
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }

    /// Reads a stream of bytes and extracts next MQTT packet out of it
    async fn parse(&self, stream: &mut BytesMut, max_size: usize) -> anyhow::Result<()> {
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
                self.senders
                    .tx_connect
                    .send(ConnAck::read(fixed_header, packet)?)?;
            }
            PacketType::Publish => {
                self.tx_publish_rel(Publish::read(fixed_header, packet)?.into())
                    .await;
            }
            PacketType::PubAck => {
                self.tx_publish_rel(PubAck::read(fixed_header, packet)?.into())
                    .await;
            }
            PacketType::PubRec => {
                self.tx_publish_rel(PubRec::read(fixed_header, packet)?.into())
                    .await;
            }
            PacketType::PubRel => {
                self.tx_publish_rel(PubRel::read(fixed_header, packet)?.into())
                    .await;
            }
            PacketType::PubComp => {
                self.tx_publish_rel(PubComp::read(fixed_header, packet)?.into())
                    .await;
            }
            // PacketType::Subscribe => Packet::Subscribe(Subscribe::read(fixed_header, packet)?),
            PacketType::SubAck => {
                self.tx_subscribe_rel(SubAck::read(fixed_header, packet)?.into())
                    .await;
            }
            // PacketType::Unsubscribe => Packet::Unsubscribe(Unsubscribe::read(fixed_header, packet)?),
            PacketType::UnsubAck => {
                self.tx_subscribe_rel(UnsubAck::read(fixed_header, packet)?.into())
                    .await;
            }
            // PacketType::PingReq => Packet::PingReq,
            PacketType::PingResp => {
                warn!("PingResp should be zero byte");
                self.senders.tx_ping.send(PingResp)?;
            }
            // PacketType::Disconnect => Packet::Disconnect,
            ty => {
                warn!("should not receive {:?}", ty);
            }
        };
        Ok(())
    }

    async fn tx_publish_rel(&self, msg: PublishMsg) {
        if self.senders.tx_publish.send(msg).await.is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_subscribe_rel(&self, msg: SubscribeMsg) {
        if self.senders.tx_subscribe.send(msg).is_err() {
            error!("fail to send subscriber");
        }
    }
    async fn tx_connect_rel(&self, msg: HubMsg) {
        if self.senders.tx_hub.send(msg).await.is_err() {
            error!("fail to send connector");
        }
    }
}
