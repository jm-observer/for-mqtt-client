use crate::tasks::Senders;
use crate::v3_1_1::{PubAck, Publish};
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxDupQos1 {
    tx: Senders,
    pkid: u16,
}

impl TaskPublishRxDupQos1 {
    pub async fn init(tx: Senders, pkid: u16) {
        spawn(async move {
            let mut publish = Self { tx, pkid };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        let data = PubAck::new(self.pkid);
        self.tx.tx_network_without_receipt(data).await.unwrap();
        debug!("rx dup publish qos 1 success");
    }
}
