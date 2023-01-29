use crate::datas::payload::Payload;
use crate::datas::trace_publish::TracePublish;
use crate::tasks::Senders;
use crate::v3_1_1::Publish;
use crate::{QoS, QoSWithPacketId};
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos0 {
    tx: Senders,
    trace_publish: TracePublish,
}

impl TaskPublishQos0 {
    pub async fn init(tx: Senders, trace_publish: TracePublish) {
        spawn(async move {
            let mut publish = Self { tx, trace_publish };
            publish.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> anyhow::Result<()> {
        debug!("start to Publish");
        let packet = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::AtMostOnce,
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
        );
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes);
        let data = bytes.freeze();
        self.tx.tx_network_default(data).await?;
        debug!("publish qos 0 success");
        self.tx.tx_to_user(self.trace_publish.id());
        Ok(())
    }
}
