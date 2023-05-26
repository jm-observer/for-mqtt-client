use crate::{
    protocol::packet::{UnsubAck, Unsubscribe},
    tasks::{
        task_client::data::{TraceUnubscribe, UnsubscribeAck},
        task_hub::HubMsg,
        utils::{complete_to_tx_packet, CommonErr},
        HubError, Senders, TIMEOUT_TO_COMPLETE_TX
    }
};
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use log::debug;
use tokio::spawn;

#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskUnsubscribe {
    tx:                Senders,
    rx:                IdentityOfSimple<UnsubAck>,
    trace_unsubscribe: TraceUnubscribe
}

impl TaskUnsubscribe {
    pub async fn init(
        bus: EntryOfBus,
        trace_unsubscribe: TraceUnubscribe
    ) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login::<Self, UnsubAck>().await?;
        let tx = rx.tx();
        spawn(async move {
            let mut unsubscribe = Self {
                tx: Senders::init(tx),
                rx,
                trace_unsubscribe
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
        // let mut rx_ack =
        // self.tx.broadcast_tx.tx_unsub_ack.subscribe();
        let ack = complete_to_tx_packet::<UnsubAck, Unsubscribe>(
            &mut self.rx,
            self.trace_unsubscribe.packet_id(),
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut self.trace_unsubscribe.unsubscribe
        )
        .await?;
        self.rx
            .dispatch_event(HubMsg::RecoverId(
                self.trace_unsubscribe.packet_id()
            ))
            .await?;
        self.tx
            .tx_to_user::<UnsubscribeAck>(UnsubscribeAck::init(
                ack.as_ref().clone(),
                self.trace_unsubscribe.id
            ))
            .await;
        Ok(())
    }
}
