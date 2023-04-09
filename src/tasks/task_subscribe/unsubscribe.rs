use crate::protocol::packet::{UnsubAck, Unsubscribe};
use crate::tasks::task_client::data::{TraceUnubscribe, UnsubscribeAck};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{HubError, Senders, TIMEOUT_TO_COMPLETE_TX};
use for_event_bus::worker::IdentityOfSimple;
use for_event_bus::CopyOfBus;
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskUnsubscribe {
    tx: Senders,
    rx: IdentityOfSimple<UnsubAck>,
    trace_unsubscribe: TraceUnubscribe,
}

impl TaskUnsubscribe {
    pub async fn init(bus: CopyOfBus, trace_unsubscribe: TraceUnubscribe) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login().await?;
        let tx = rx.tx();
        spawn(async move {
            let mut unsubscribe = Self {
                tx: Senders::init(tx),
                rx,
                trace_unsubscribe,
            };
            if let Err(e) = unsubscribe.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
        Ok(())
    }
    async fn run(&mut self) -> anyhow::Result<(), CommonErr> {
        debug!("start to unsubscribe");
        // self.rx.subscribe::<UnsubAck>()?;
        // let mut rx_ack = self.tx.broadcast_tx.tx_unsub_ack.subscribe();
        let ack = complete_to_tx_packet::<UnsubAck, Unsubscribe>(
            &mut self.rx,
            self.trace_unsubscribe.packet_id(),
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut self.trace_unsubscribe.unsubscribe,
        )
        .await?;
        self.rx
            .dispatch_event(HubMsg::RecoverId(self.trace_unsubscribe.packet_id()))?;
        self.tx.tx_to_user::<UnsubscribeAck>(UnsubscribeAck::init(
            ack.as_ref().clone(),
            self.trace_unsubscribe.id,
        ));
        Ok(())
    }
}
