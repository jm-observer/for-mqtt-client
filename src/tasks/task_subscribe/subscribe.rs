use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::utils::complete_to_tx_packet;
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::v3_1_1::Subscribe;
use crate::QoS;
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskSubscribe {
    tx: Senders,
    topic: Arc<String>,
    qos: QoS,
    packet_id: u16,
}

impl TaskSubscribe {
    pub fn init<T: Into<Arc<String>>>(tx: Senders, topic: T, qos: QoS, packet_id: u16) {
        let topic = topic.into();
        spawn(async move {
            let mut subscriber = Self {
                tx,
                topic,
                qos,
                packet_id,
            };
            subscriber.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> anyhow::Result<()> {
        debug!("start to subscribe");
        let mut packet = Subscribe::new(self.topic.clone(), self.qos.clone(), self.packet_id);
        let mut rx_ack = self.tx.broadcast_tx.tx_sub_ack.subscribe();
        complete_to_tx_packet(
            &mut rx_ack,
            self.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut packet,
        )
        .await?;
        self.tx
            .tx_hub
            .send(HubMsg::RecoverId(self.packet_id))
            .await
            .unwrap();
        Ok(())
    }
}
