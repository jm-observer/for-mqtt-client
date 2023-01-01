mod task_hub;
mod task_keep_alive;
mod task_network;
mod task_ping;
mod task_publish;
mod task_subscribe;

use bytes::Bytes;
use log::{error, warn};
use std::sync::Arc;
pub use task_hub::TaskHub;
pub use task_publish::TaskPublishMg;
pub use task_subscribe::TaskSubscriber;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_network::{Data, DataWaitingToBeSend};
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::v3_1_1::{ConnAck, PingResp};
use anyhow::Result;
use tokio::sync::broadcast::*;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Senders {
    tx_network: mpsc::Sender<Data>,
    tx_publish: mpsc::Sender<PublishMsg>,
    tx_subscribe: Sender<SubscribeMsg>,
    tx_ping: Sender<PingResp>,
    tx_connect: Sender<ConnAck>,
    tx_user: Sender<MqttEvent>,
    tx_hub: mpsc::Sender<HubMsg>,
}

impl Senders {
    pub fn init(
        tx_network_writer: mpsc::Sender<Data>,
        tx_publisher: mpsc::Sender<PublishMsg>,
        tx_subscriber: Sender<SubscribeMsg>,
        tx_user: Sender<MqttEvent>,
        tx_hub: mpsc::Sender<HubMsg>,
        tx_ping: Sender<PingResp>,
        tx_connect: Sender<ConnAck>,
    ) -> Self {
        Self {
            tx_subscribe: tx_subscriber,
            tx_publish: tx_publisher,
            tx_network: tx_network_writer,
            tx_ping,
            tx_user,
            tx_hub,
            tx_connect,
        }
    }
    pub fn rx_user(&self) -> Receiver<MqttEvent> {
        self.tx_user.subscribe()
    }
    pub fn tx_mqtt_event(&self, event: MqttEvent) {
        if self.tx_user.send(event).is_err() {
            warn!("fail to tx mqtt event")
        }
    }
    pub fn subscribe_ping(&self) -> Receiver<PingResp> {
        self.tx_ping.subscribe()
    }
    pub fn subscribe_connect(&self) -> Receiver<ConnAck> {
        self.tx_connect.subscribe()
    }
    pub async fn tx_network_default<T: Into<Arc<Bytes>>>(
        &self,
        bytes: T,
    ) -> Result<oneshot::Receiver<Receipt>> {
        let (receipter, rx) = Receipter::default();
        self.tx_network
            .send(
                DataWaitingToBeSend {
                    data: bytes.into(),
                    receipter,
                }
                .into(),
            )
            .await?;
        Ok(rx)
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
}
