use crate::tasks::Senders;
use crate::v3_1_1::{PubAck, PubComp, Publish};
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxDupQos2 {
    tx: Senders,
    pkid: u16,
}

impl TaskPublishRxDupQos2 {
    pub async fn init_by_rel(tx: Senders, pkid: u16) {
        spawn(async move {
            let mut publish = Self { tx, pkid };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        let data = PubComp::new(self.pkid);
        self.tx.tx_network_without_receipt(data).await.unwrap();
        debug!("rx dup publish qos 2 success");
    }
}
