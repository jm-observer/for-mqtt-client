use crate::tasks::Senders;
use crate::v3_1_1::{PubAck, Publish};
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos1 {
    tx: Senders,
    topic: Arc<String>,
    payload: Bytes,
    qos: QoS,
    pkid: u16,
}

impl TaskPublishRxQos1 {
    pub async fn init(tx: Senders, topic: Arc<String>, payload: Bytes, qos: QoS, pkid: u16) {
        spawn(async move {
            let mut publish = Self {
                tx,
                topic,
                payload,
                qos,
                pkid,
            };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        let data = PubAck::new(self.pkid).unwrap();
        self.tx.tx_network_without_receipt(data).await.unwrap();
        // todo send publish to user or hub
        debug!("rx publish qos 1 success");
    }
}
