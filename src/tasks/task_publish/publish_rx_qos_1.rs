use crate::tasks::task_hub::HubMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{PubAck, Publish};
use crate::QoS;
use anyhow::Result;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos1 {
    tx: Senders,
    packet_id: u16,
}

impl TaskPublishRxQos1 {
    pub fn init(tx: Senders, packet_id: u16) {
        spawn(async move {
            let mut publish = Self { tx, packet_id };
            publish.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> Result<()> {
        let data = PubAck::data(self.packet_id);
        self.tx.tx_network_default(data).await?;
        self.tx
            .tx_hub
            .send(HubMsg::AffirmRxId(self.packet_id))
            .await?;
        Ok(())
    }
}
