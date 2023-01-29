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
    topic: Arc<String>,
    payload: Bytes,
    qos: QoS,
    retain: bool,
}

impl TaskPublishQos0 {
    pub async fn init(tx: Senders, topic: Arc<String>, payload: Bytes, qos: QoS, retain: bool) {
        spawn(async move {
            let mut publish = Self {
                tx,
                topic,
                payload,
                qos,
                retain,
            };
            publish.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> anyhow::Result<()> {
        debug!("start to Publish");
        let packet = Publish::new(
            self.topic.clone(),
            QoSWithPacketId::AtMostOnce,
            self.payload.clone(),
            self.retain,
        )?;
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes);
        let data = bytes.freeze();
        self.tx.tx_network_default(data).await?;
        debug!("publish qos 0 success");
        Ok(())
    }
}
