use crate::tasks::task_client::data::{TraceUnubscribe, UnsubscribeAck};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskUnsubscribe {
    tx: Senders,
    trace_unsubscribe: TraceUnubscribe,
}

impl TaskUnsubscribe {
    pub fn init(tx: Senders, trace_unsubscribe: TraceUnubscribe) {
        spawn(async move {
            let mut unsubscribe = Self {
                tx,
                trace_unsubscribe,
            };
            if let Err(e) = unsubscribe.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> anyhow::Result<(), CommonErr> {
        debug!("start to unsubscribe");
        let mut rx_ack = self.tx.broadcast_tx.tx_unsub_ack.subscribe();
        let ack = complete_to_tx_packet(
            &mut rx_ack,
            self.trace_unsubscribe.packet_id(),
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut self.trace_unsubscribe.unsubscribe,
        )
        .await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::RecoverId(self.trace_unsubscribe.packet_id()))
            .await?;
        self.tx
            .tx_to_user::<UnsubscribeAck>(UnsubscribeAck::init(ack, self.trace_unsubscribe.id));
        Ok(())
    }
}
