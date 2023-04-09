use crate::protocol::packet::{PubAck, Publish};
use crate::tasks::task_client::data::TracePublishQos;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{HubError, Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::{AtLeastOnce, QoSWithPacketId};
use anyhow::Result;
use for_event_bus::worker::{IdentityOfRx, IdentityOfSimple};
use for_event_bus::CopyOfBus;
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos1 {
    tx: Senders,
    rx: IdentityOfSimple<PubAck>,
    trace_publish: TracePublishQos<AtLeastOnce>,
}

impl TaskPublishQos1 {
    pub async fn init(
        bus: CopyOfBus,
        trace_publish: TracePublishQos<AtLeastOnce>,
    ) -> Result<(), HubError> {
        let rx = bus.simple_login().await?;
        let tx = rx.tx();
        spawn(async move {
            let mut publish = Self {
                tx: Senders::init(tx),
                rx,
                trace_publish,
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
            QoSWithPacketId::AtLeastOnce(self.trace_publish.packet_id),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
            self.trace_publish.protocol,
        );
        // let mut rx_ack = self.senders.broadcast_tx.tx_pub_ack.subscribe();
        complete_to_tx_packet::<PubAck, Publish>(
            &mut self.rx,
            self.trace_publish.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut packet,
        )
        .await?;
        debug!("publish qos 1 success");
        self.rx
            .dispatch_event(HubMsg::RecoverId(self.trace_publish.packet_id))?;
        // self.tx
        //     .tx_hub_msg
        //     .send()
        //     .await?;
        self.rx
            .dispatch_event(HubMsg::RecoverId(self.trace_publish.packet_id))?;

        self.tx.tx_to_user(self.trace_publish.id());
        Ok(())
    }
}
