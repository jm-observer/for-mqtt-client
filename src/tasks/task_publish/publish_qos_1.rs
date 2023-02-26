use crate::tasks::task_client::data::TracePublishQos;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::v3_1_1::Publish;
use crate::{AtLeastOnce, QoSWithPacketId};
use anyhow::Result;
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos1 {
    senders: Senders,
    trace_publish: TracePublishQos<AtLeastOnce>,
}

impl TaskPublishQos1 {
    pub async fn init(tx: Senders, trace_publish: TracePublishQos<AtLeastOnce>) {
        spawn(async move {
            let mut publish = Self {
                senders: tx,
                trace_publish,
            };
            if let Err(e) = publish.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> Result<(), CommonErr> {
        debug!("start to Publish");
        let mut packet = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::AtLeastOnce(self.trace_publish.packet_id),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
        );
        let mut rx_ack = self.senders.broadcast_tx.tx_pub_ack.subscribe();
        complete_to_tx_packet(
            &mut rx_ack,
            self.trace_publish.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.senders,
            &mut packet,
        )
        .await?;
        debug!("publish qos 1 success");
        self.senders
            .tx_hub_msg
            .send(HubMsg::RecoverId(self.trace_publish.packet_id))
            .await?;
        self.senders.tx_to_user(self.trace_publish.id());
        Ok(())
    }
}
