use crate::{
    protocol::packet::{PubAck, Publish},
    tasks::{
        task_client::data::TracePublishQos,
        task_hub::HubMsg,
        utils::{complete_to_tx_packet, CommonErr},
        HubError, Senders, TIMEOUT_TO_COMPLETE_TX
    },
    AtLeastOnce, QoSWithPacketId
};
use anyhow::Result;
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
#[derive(Worker)]
pub struct TaskPublishQos1 {
    tx:            Senders,
    rx:            IdentityOfSimple<PubAck>,
    trace_publish: TracePublishQos<AtLeastOnce>
}

impl TaskPublishQos1 {
    pub async fn init(
        bus: EntryOfBus,
        trace_publish: TracePublishQos<AtLeastOnce>
    ) -> Result<(), HubError> {
        let rx = bus.simple_login::<Self, PubAck>().await?;
        let tx = rx.tx();
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
        let mut packet = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::AtLeastOnce(
                self.trace_publish.packet_id
            ),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
            self.trace_publish.protocol
        );
        // let mut rx_ack =
        // self.senders.broadcast_tx.tx_pub_ack.subscribe();
        complete_to_tx_packet::<PubAck, Publish>(
            &mut self.rx,
            self.trace_publish.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut packet
        )
        .await?;
        debug!("publish qos 1 success");
        self.rx
            .dispatch_event(HubMsg::RecoverId(
                self.trace_publish.packet_id
            ))
            .await?;
        // self.tx
        //     .tx_hub_msg
        //     .send()
        //     .await?;
        self.rx
            .dispatch_event(HubMsg::RecoverId(
                self.trace_publish.packet_id
            ))
            .await?;

        self.tx.tx_to_user(self.trace_publish.id()).await;
        Ok(())
    }
}
