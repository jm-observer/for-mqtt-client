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
pub use task_hub::{HubError, HubMsg, TaskHub};
pub use task_subscribe::TaskSubscribe;

use crate::tasks::task_network::DataWaitingToBeSend;
use crate::tasks::utils::CommonErr;
use anyhow::Result;
use for_event_bus::worker::IdentityOfTx;
use task_client::data::MqttEvent;
use tokio::sync::broadcast::*;

pub const TIMEOUT_TO_COMPLETE_TX: u64 = 10;
//
// #[derive(Clone)]
// pub struct BroadcastTx {
//     tx_pub_ack: Sender<PubAck>,
//     tx_pub_rec: Sender<PubRec>,
//     tx_pub_rel: Sender<PubRel>,
//     tx_pub_comp: Sender<PubComp>,
//     tx_sub_ack: Sender<SubAck>,
//     tx_unsub_ack: Sender<UnsubAck>,
//     tx_ping: Sender<PingResp>,
// }
//
// impl BroadcastTx {
//     pub fn init(capacity: usize) -> Self {
//         let (tx_pub_ack, _) = channel(capacity);
//         let (tx_pub_rec, _) = channel(capacity);
//         let (tx_pub_rel, _) = channel(capacity);
//         let (tx_pub_comp, _) = channel(capacity);
//         let (tx_sub_ack, _) = channel(capacity);
//         let (tx_unsub_ack, _) = channel(capacity);
//         let (tx_ping, _) = channel(capacity);
//         Self {
//             tx_pub_ack,
//             tx_pub_rec,
//             tx_pub_rel,
//             tx_pub_comp,
//             tx_sub_ack,
//             tx_unsub_ack,
//             tx_ping,
//         }
//     }
// }

#[derive(Clone)]
pub struct Senders {
    // tx_hub_msg: mpsc::Sender<HubMsg>,
    // tx_hub_network_event: mpsc::Sender<NetworkEvent>,
    // tx_network_data: mpsc::Sender<DataWaitingToBeSend>,
    // tx_hub_network_command: mpsc::Sender<HubNetworkCommand>,
    // broadcast_tx: BroadcastTx,
    tx: IdentityOfTx,
}

impl Senders {
    pub fn init(tx: IdentityOfTx) -> Self {
        // let (tx_hub_msg, rx_hub_msg) = mpsc::channel(buffer);
        // let (tx_hub_network_event, rx_hub_network_event) = mpsc::channel(buffer);
        // let (tx_network_data, rx_network_data) = mpsc::channel(buffer);
        // let (tx_hub_network_command, rx_hub_network_command) = mpsc::channel(buffer);
        // (
        Self { tx }
        //     rx_hub_msg,
        //     rx_hub_network_event,
        //     rx_network_data,
        //     rx_hub_network_command,
        // )
    }
    pub fn tx_to_user<T: Into<MqttEvent>>(&self, msg: T) {
        if self.tx.dispatch_event(msg.into()).is_err() {
            warn!("fail to tx mqtt event")
        }
    }
    // pub fn subscribe_ping(&self) -> Receiver<PingResp> {
    //     self.broadcast_tx.tx_ping.subscribe()
    // }
    pub async fn tx_network_default<T: Into<Arc<Bytes>>>(
        &self,
        bytes: T,
    ) -> Result<Receipt, CommonErr> {
        let (receipter, mut rx) = Receipter::default();
        self.tx
            .dispatch_event(DataWaitingToBeSend::init(bytes.into(), Some(receipter)))?;
        Ok(rx.recv().await?)
    }
    // pub async fn tx_network_without_receipt<T: Into<Arc<Bytes>>>(&self, bytes: T) -> Result<()> {
    //     self.tx_network_data
    //         .send(DataWaitingToBeSend::init(bytes.into(), None).into())
    //         .await?;
    //     Ok(())
    // }
}
#[derive(Debug)]
pub struct Receipter {
    val: Receipt,
    tx: Sender<Receipt>,
}
#[derive(Debug, Clone)]
pub enum Receipt {
    None,
}

impl Receipter {
    pub fn default() -> (Self, Receiver<Receipt>) {
        Self::init(Receipt::None)
    }
    pub fn init(val: Receipt) -> (Self, Receiver<Receipt>) {
        let (tx, rx) = channel(1);
        (Self { val, tx }, rx)
    }
    pub fn done(&self) {
        if self.tx.send(self.val.clone()).is_err() {
            error!("fail to send receipt")
        }
    }
}
