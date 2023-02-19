use crate::tasks::task_client::data::{SubscribeAck, SubscribeFilterAck, TraceSubscribe};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::utils::complete_to_tx_packet;
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::v3_1_1::{SubAck, Subscribe, SubscribeFilter};
use crate::QoS;
use anyhow::bail;
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskSubscribe {
    tx: Senders,
    trace_packet: TraceSubscribe,
}

impl TaskSubscribe {
    pub fn init(tx: Senders, trace_packet: TraceSubscribe) {
        spawn(async move {
            let mut subscriber = Self { tx, trace_packet };
            subscriber.run().await.unwrap();
        });
    }
    async fn run(self) -> anyhow::Result<()> {
        let TaskSubscribe { tx, trace_packet } = self;
        debug!("start to subscribe");
        let mut packet = Subscribe::new(trace_packet.filters.clone(), trace_packet.packet_id);
        let mut rx_ack = tx.broadcast_tx.tx_sub_ack.subscribe();
        let ack = complete_to_tx_packet(
            &mut rx_ack,
            trace_packet.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &tx,
            &mut packet,
        )
        .await?;
        let SubAck { return_codes, .. } = ack;
        tx.tx_hub_msg
            .send(HubMsg::RecoverId(trace_packet.packet_id))
            .await
            .unwrap();

        let TraceSubscribe { id, filters, .. } = trace_packet;
        if return_codes.len() != filters.len() {
            bail!("todo");
        }
        let filter_ack: Vec<SubscribeFilterAck> = return_codes
            .into_iter()
            .zip(filters.into_iter())
            .map(|(ack, filter)| {
                let SubscribeFilter { path, .. } = filter;
                SubscribeFilterAck { path, ack }
            })
            .collect();
        let ack = SubscribeAck { id, filter_ack };
        tx.tx_to_user(ack);
        Ok(())
    }
}
