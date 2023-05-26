use crate::{
    protocol::packet::{PubRec, Publish},
    tasks::{
        task_client::data::TracePublishQos,
        task_hub::HubMsg,
        utils::{complete_to_tx_packet, CommonErr},
        HubError, Senders, TIMEOUT_TO_COMPLETE_TX
    },
    ExactlyOnce, QoSWithPacketId
};
use anyhow::Result;
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use log::debug;
use tokio::spawn;
#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos2 {
    tx:            Senders,
    rx:            IdentityOfSimple<PubRec>,
    trace_publish: TracePublishQos<ExactlyOnce>
}

impl TaskPublishQos2 {
    pub async fn init(
        bus: EntryOfBus,
        trace_publish: TracePublishQos<ExactlyOnce>
    ) -> Result<(), HubError> {
        let rx = bus.simple_login::<Self, PubRec>().await?;
        let tx = rx.tx();
        // let (tx, rx) = bus.login().await?;
        spawn(async move {
            let mut publish = Self {
                tx: Senders::init(tx),
                rx,
                trace_publish
            };
            if let Err(e) = publish.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
        Ok(())
    }

    async fn run(&mut self) -> Result<(), CommonErr> {
        debug!("start to Publish");
        let mut data = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::ExactlyOnce(
                self.trace_publish.packet_id
            ),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
            self.trace_publish.protocol
        );
        // let mut rx_ack =
        // self.tx.broadcast_tx.tx_pub_rec.subscribe();
        // self.rx.subscribe::<PubRec>()?;
        complete_to_tx_packet::<PubRec, Publish>(
            &mut self.rx,
            self.trace_publish.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut data
        )
        .await?;
        self.rx
            .dispatch_event(HubMsg::RecoverId(
                self.trace_publish.packet_id
            ))
            .await?;
        Ok(())
    }
}
