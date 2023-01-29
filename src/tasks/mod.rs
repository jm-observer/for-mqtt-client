pub(crate) mod task_client;
mod task_hub;
mod task_network;
mod task_ping;
mod task_publish;
mod task_subscribe;
mod utils;

use bytes::Bytes;
use log::{error, warn};
use std::sync::Arc;
pub use task_hub::TaskHub;
pub use task_subscribe::TaskSubscribe;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_network::{Data, DataWaitingToBeSend};
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::v3_1_1::{
    ConnAck, PingResp, PubAck, PubComp, PubRec, PubRel, Publish, SubAck, UnsubAck,
};
use anyhow::Result;
use tokio::sync::broadcast::*;
use tokio::sync::{mpsc, oneshot};

pub const TIMEOUT_TO_COMPLETE_TX: u64 = 10;

#[derive(Clone)]
pub struct BroadcastTx {
    tx_publish: Sender<Publish>,
    tx_pub_ack: Sender<PubAck>,
    tx_pub_rec: Sender<PubRec>,
    tx_pub_rel: Sender<PubRel>,
    tx_pub_comp: Sender<PubComp>,
    tx_sub_ack: Sender<SubAck>,
    tx_unsub_ack: Sender<UnsubAck>,
    tx_ping: Sender<PingResp>,
    tx_connect: Sender<ConnAck>,
    tx_user: Sender<MqttEvent>,
}

impl BroadcastTx {
    pub fn init(capacity: usize) -> Self {
        let (tx_publish, _) = channel(capacity);
        let (tx_pub_ack, _) = channel(capacity);
        let (tx_pub_rec, _) = channel(capacity);
        let (tx_pub_rel, _) = channel(capacity);
        let (tx_pub_comp, _) = channel(capacity);
        let (tx_sub_ack, _) = channel(capacity);
        let (tx_unsub_ack, _) = channel(capacity);
        let (tx_ping, _) = channel(capacity);
        let (tx_connect, _) = channel(capacity);
        let (tx_user, _) = channel(capacity);
        Self {
            tx_publish,
            tx_pub_ack,
            tx_pub_rec,
            tx_pub_rel,
            tx_pub_comp,
            tx_sub_ack,
            tx_unsub_ack,
            tx_ping,
            tx_connect,
            tx_user,
        }
    }
}

#[derive(Clone)]
pub struct Senders {
    tx_network: mpsc::Sender<Data>,
    tx_hub: mpsc::Sender<HubMsg>,
    broadcast_tx: BroadcastTx,
}

impl Senders {
    pub fn init(tx_network_writer: mpsc::Sender<Data>, tx_hub: mpsc::Sender<HubMsg>) -> Self {
        Self {
            tx_network: tx_network_writer,
            tx_hub,
            broadcast_tx: BroadcastTx::init(1024),
        }
    }
    pub fn rx_user(&self) -> Receiver<MqttEvent> {
        self.broadcast_tx.tx_user.subscribe()
    }
    pub fn tx_to_user<T: Into<MqttEvent>>(&self, msg: T) {
        if self.broadcast_tx.tx_user.send(msg.into()).is_err() {
            warn!("fail to tx mqtt event")
        }
    }
    pub fn subscribe_ping(&self) -> Receiver<PingResp> {
        self.broadcast_tx.tx_ping.subscribe()
    }
    pub fn subscribe_connect(&self) -> Receiver<ConnAck> {
        self.broadcast_tx.tx_connect.subscribe()
    }
    pub async fn tx_network_default<T: Into<Arc<Bytes>>>(&self, bytes: T) -> Result<Receipt> {
        let (receipter, rx) = Receipter::default();
        self.tx_network
            .send(DataWaitingToBeSend::init(bytes.into(), Some(receipter)).into())
            .await?;
        Ok(rx.await?)
    }
    pub async fn tx_network_without_receipt<T: Into<Arc<Bytes>>>(&self, bytes: T) -> Result<()> {
        self.tx_network
            .send(DataWaitingToBeSend::init(bytes.into(), None).into())
            .await?;
        Ok(())
    }
}
#[derive(Debug)]
pub struct Receipter {
    val: Receipt,
    tx: oneshot::Sender<Receipt>,
}
#[derive(Debug, Clone)]
pub enum Receipt {
    None,
}

impl Receipter {
    pub fn default() -> (Self, oneshot::Receiver<Receipt>) {
        Self::init(Receipt::None)
    }
    pub fn init(val: Receipt) -> (Self, oneshot::Receiver<Receipt>) {
        let (tx, rx) = oneshot::channel();
        (Self { val, tx }, rx)
    }
    pub fn done(self) {
        if self.tx.send(self.val).is_err() {
            error!("fail to send receipt")
        }
    }
}

#[derive(Debug, Clone)]
pub enum UserMsg {}

#[derive(Debug, Clone)]
pub enum MqttEvent {
    ConnectSuccess,
    ConnectFail(String),
    Publish(Publish),
}

impl From<Publish> for MqttEvent {
    fn from(msg: Publish) -> Self {
        MqttEvent::Publish(msg)
    }
}
