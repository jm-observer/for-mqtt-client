use crate::v3_1_1::*;
use bytes::BytesMut;
use log::{debug, error, warn};
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf};
use tokio::net::TcpStream;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::mpsc;
use url::Url;

mod data;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publisher::PublishMsg;
use crate::tasks::task_subscriber::SubscribeMsg;
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
    tx_to_hub: mpsc::Sender<NetworkMsg>,
}

impl TaskNetwork {
    pub fn init(
        addr: String,
        port: u16,
        inner_tx: Senders,
        rx: mpsc::Receiver<NetworkData>,
        tx_to_hub: mpsc::Sender<NetworkMsg>,
    ) -> Self {
        let (sub_task_tx, _) = channel(1024);
        Self {
            addr,
            port,
            senders: inner_tx,
            sub_task_tx,
            rx,
            tx_to_hub,
        }
    }
    pub async fn run(mut self) {
        tokio::spawn(async move {
            debug!("{}: {}", self.addr, self.port);
            let stream = TcpStream::connect((self.addr.as_str(), self.port))
                .await
                .unwrap();
            if let Err(e) = self.tx_to_hub.send(NetworkMsg::NetworkConnectSuccess).await {
                error!("{:?}", e);
            }
            let (reader, mut writer) = tokio::io::split(stream);
            SubTaskNetworkReader::init(
                self.senders.clone(),
                reader,
                self.sub_task_tx.clone(),
                self.tx_to_hub.clone(),
            )
            .await;
            let mut buf = BytesMut::with_capacity(1024);
            loop {
                match self.rx.recv().await {
                    Some(val) => {
                        let Err(e) = writer.write_all(val.as_ref().as_ref()).await else {
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
}

struct SubTaskNetworkReader {
    senders: Senders,
    reader: ReadHalf<TcpStream>,
    sub_task_tx: Sender<Command>,
    tx_to_hub: mpsc::Sender<NetworkMsg>,
}

impl SubTaskNetworkReader {
    async fn init(
        senders: Senders,
        reader: ReadHalf<TcpStream>,
        sub_task_tx: Sender<Command>,
        tx_to_hub: mpsc::Sender<NetworkMsg>,
    ) {
        let reader = Self {
            senders,
            reader,
            sub_task_tx,
            tx_to_hub,
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
            let read = self.reader.read_buf(&mut buf).await.unwrap();
            if 0 == read {
                warn!("");
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
        if fixed_header.remaining_len() == 0 {
            // no payload packets
            return match packet_type {
                PacketType::PingReq => {
                    warn!("should not receive pingreq");
                    Ok(())
                }
                PacketType::PingResp => {
                    // self.tx_connect_rel(HubMsg::PingResp).await;
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
                self.tx_to_hub
                    .send(NetworkMsg::ConnAck(ConnAck::read(fixed_header, packet)?))
                    .await?;
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
                self.tx_to_hub.send(NetworkMsg::PingResp).await?;
            }
            // PacketType::Disconnect => Packet::Disconnect,
            ty => {
                warn!("should not receive {:?}", ty);
            }
        };
        Ok(())
    }

    async fn tx_publish_rel(&self, msg: PublishMsg) {
        if self.senders.tx_publisher.send(msg).await.is_err() {
            error!("fail to send publisher");
        }
    }
    async fn tx_subscribe_rel(&self, msg: SubscribeMsg) {
        if self.senders.tx_subscriber.send(msg).is_err() {
            error!("fail to send subscriber");
        }
    }
    async fn tx_connect_rel(&self, msg: HubMsg) {
        if self.senders.tx_hub.send(msg).await.is_err() {
            error!("fail to send connector");
        }
    }
}
