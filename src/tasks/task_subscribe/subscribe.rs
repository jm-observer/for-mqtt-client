use crate::{
    protocol::packet::{SubAck, Subscribe},
    tasks::{
        task_client::data::TraceSubscribe,
        task_hub::HubMsg,
        utils::{complete_to_tx_packet, CommonErr},
        HubError, Senders, TIMEOUT_TO_COMPLETE_TX
    },
    SubscribeAck
};
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use tokio::spawn;

#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskSubscribe {
    tx:           Senders,
    rx:           IdentityOfSimple<SubAck>,
    trace_packet: TraceSubscribe
}

impl TaskSubscribe {
    pub async fn init(
        bus: EntryOfBus,
        trace_packet: TraceSubscribe
    ) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login::<Self, SubAck>().await?;
        let tx = rx.tx();
        spawn(async move {
            let subscriber = Self {
                tx: Senders::init(tx),
                rx,
                trace_packet
            };
            if let Err(e) = subscriber.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
        Ok(())
    }

    async fn run(self) -> anyhow::Result<(), CommonErr> {
        let TaskSubscribe {
            tx,
            mut rx,
            mut trace_packet
        } = self;
        // debug!("start to subscribe");
        // rx.subscribe::<SubAck>()?;
        let ack = complete_to_tx_packet::<SubAck, Subscribe>(
            &mut rx,
            trace_packet.packet_id(),
            TIMEOUT_TO_COMPLETE_TX,
            &tx,
            &mut trace_packet.subscribe
        )
        .await?;

        rx.dispatch_event(HubMsg::RecoverId(
            trace_packet.packet_id()
        ))
        .await?;

        // let SubAck { return_codes, .. } = ack;
        let TraceSubscribe { id, .. } = trace_packet;
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
        let ack = SubscribeAck {
            id,
            acks: ack.as_ref().clone().return_codes()
        };
        tx.tx_to_user(ack).await;
        Ok(())
    }
}
