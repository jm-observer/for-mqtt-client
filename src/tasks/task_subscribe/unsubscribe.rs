use crate::tasks::task_client::data::{TraceUnubscribe, UnsubscribeAck};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::utils::complete_to_tx_packet;
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::v3_1_1::{Subscribe, Unsubscribe};
use crate::QoS;
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskUnsubscribe {
    tx: Senders,
    trace_unsubscribe: TraceUnubscribe,
    packet_id: u16,
}

impl TaskUnsubscribe {
    pub fn init(tx: Senders, trace_unsubscribe: TraceUnubscribe, packet_id: u16) {
        spawn(async move {
            let mut unsubscribe = Self {
                tx,
                trace_unsubscribe,
                packet_id,
            };
            unsubscribe.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> anyhow::Result<()> {
        debug!("start to unsubscribe");
        let mut packet = Unsubscribe::new(self.trace_unsubscribe.topics.clone(), self.packet_id);
        let mut rx_ack = self.tx.broadcast_tx.tx_unsub_ack.subscribe();
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
        self.tx
            .tx_to_user::<UnsubscribeAck>(self.trace_unsubscribe.clone().into());
        Ok(())
    }
}
