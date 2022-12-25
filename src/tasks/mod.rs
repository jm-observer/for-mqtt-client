mod task_id_mg;
mod task_network;
mod task_hub;
mod task_connector;
mod task_publisher;
mod task_subscriber;

use bytes::Bytes;
use log::error;
pub use task_hub::*;


use tokio::sync::broadcast::*;
use tokio::sync::{mpsc, oneshot};
use crate::tasks::task_connector::ConnectMsg;
use crate::tasks::task_network::NetworkData;
use crate::tasks::task_publisher::PublishMsg;
use crate::tasks::task_subscriber::SubscribeMsg;

pub type RxBroadcast = Receiver<InnerCommand>;
#[derive(Clone)]
pub struct Senders {
    tx_network_writer: mpsc::Sender<NetworkData>,
    tx_connector: mpsc::Sender<ConnectMsg>,
    tx_publisher: mpsc::Sender<PublishMsg>,
    tx_subscriber: mpsc::Sender<SubscribeMsg>,
    tx_broadcast: Sender<InnerCommand>,
}

impl Senders {
    pub fn init(tx_network_writer: mpsc::Sender<NetworkData>,
                tx_connector: mpsc::Sender<ConnectMsg>,
                tx_publisher: mpsc::Sender<PublishMsg>,
                tx_subscriber: mpsc::Sender<SubscribeMsg>,
                tx_broadcast: Sender<InnerCommand>,) -> Self {
        Self {
            tx_subscriber, tx_connector, tx_publisher, tx_network_writer, tx_broadcast
        }
    }
    pub fn subscribe_broadcast(&self) -> RxBroadcast {
        self.tx_broadcast.subscribe()
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
    Id(u64),
}

impl Receipter {
    pub fn init(val: Receipt) -> (Self, oneshot::Receiver<Receipt>) {
        let (tx, rx) = oneshot::channel();
        (Self {
            val,
            tx
        }, rx)
    }
    pub fn done(self) {
        if self.tx.send(self.val).is_err() {
           error!("fail to send receipt")
        }
    }

}


#[derive(Debug, Clone)]
pub enum UserMsg {
}

#[derive(Debug, Clone)]
pub enum MqttEvent {
}

#[derive(Debug, Clone)]
pub enum InnerCommand {
    NetworkConnectSuccess,
    NetworkConnectFail(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InnerRole {
    Writer,
}


