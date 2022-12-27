mod task_hub;
mod task_network;
mod task_publisher;
mod task_subscriber;

use bytes::Bytes;
use log::{error, warn};
use std::sync::Arc;
pub use task_hub::TaskHub;
pub use task_publisher::TaskPublishMg;
pub use task_subscriber::TaskSubscriber;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_network::NetworkData;
use crate::tasks::task_publisher::PublishMsg;
use crate::tasks::task_subscriber::SubscribeMsg;
use anyhow::Result;
use tokio::sync::broadcast::*;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct Senders {
    tx_network_writer: mpsc::Sender<NetworkData>,
    tx_publisher: mpsc::Sender<PublishMsg>,
    tx_subscriber: Sender<SubscribeMsg>,
    tx_user: Sender<MqttEvent>,
    tx_hub: mpsc::Sender<HubMsg>,
}

impl Senders {
    pub fn init(
        tx_network_writer: mpsc::Sender<NetworkData>,
        tx_publisher: mpsc::Sender<PublishMsg>,
        tx_subscriber: Sender<SubscribeMsg>,
        tx_user: Sender<MqttEvent>,
        tx_hub: mpsc::Sender<HubMsg>,
    ) -> Self {
        Self {
            tx_subscriber,
            tx_publisher,
            tx_network_writer,
            tx_user,
            tx_hub,
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
    pub async fn tx_network_default<T: Into<Arc<Bytes>>>(
        &self,
        bytes: T,
    ) -> Result<oneshot::Receiver<Receipt>> {
        let (receipter, rx) = Receipter::default();
        self.tx_network_writer
            .send(NetworkData {
                data: bytes.into(),
                receipter,
            })
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
