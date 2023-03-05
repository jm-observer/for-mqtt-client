use crate::tasks::task_client::data::{SubscribeAck, SubscribeFilterAck, TraceSubscribe};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use log::{debug, warn};
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskSubscribe {
    tx: Senders,
    trace_packet: TraceSubscribe,
}

impl TaskSubscribe {
    pub fn init(tx: Senders, trace_packet: TraceSubscribe) {
        spawn(async move {
            let subscriber = Self { tx, trace_packet };
            if let Err(e) = subscriber.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(self) -> anyhow::Result<(), CommonErr> {
        let TaskSubscribe {
            tx,
            mut trace_packet,
        } = self;
        debug!("start to subscribe");
        let mut rx_ack = tx.broadcast_tx.tx_sub_ack.subscribe();
        let ack = complete_to_tx_packet(
            &mut rx_ack,
            trace_packet.packet_id(),
            TIMEOUT_TO_COMPLETE_TX,
            &tx,
            &mut trace_packet.subscribe,
        )
        .await?;

        tx.tx_hub_msg
            .send(HubMsg::RecoverId(trace_packet.packet_id()))
            .await?;

        todo!();
        // let SubAck { return_codes, .. } = ack;
        // let TraceSubscribe { id, filters, .. } = trace_packet;
        // if return_codes.len() != filters.len() {
        //     warn!(
        //         "filters.len {} not equal return_codes.len {}",
        //         filters.len(),
        //         return_codes.len()
        //     );
        // }
        // let filter_ack: Vec<SubscribeFilterAck> = return_codes
        //     .into_iter()
        //     .zip(filters.into_iter())
        //     .map(|(ack, filter)| {
        //         let SubscribeFilter { path, .. } = filter;
        //         SubscribeFilterAck { path, ack }
        //     })
        //     .collect();
        // let ack = SubscribeAck { id, filter_ack };
        // tx.tx_to_user(ack);
        Ok(())
    }
}
