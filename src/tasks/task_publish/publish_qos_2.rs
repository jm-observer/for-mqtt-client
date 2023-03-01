use crate::protocol::packet::publish::Publish;
use crate::tasks::task_client::data::TracePublishQos;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::{ExactlyOnce, QoSWithPacketId};
use anyhow::Result;
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos2 {
    tx: Senders,
    trace_publish: TracePublishQos<ExactlyOnce>,
}

impl TaskPublishQos2 {
    pub async fn init(tx: Senders, trace_publish: TracePublishQos<ExactlyOnce>) {
        spawn(async move {
            let mut publish = Self { tx, trace_publish };
            if let Err(e) = publish.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> Result<(), CommonErr> {
        debug!("start to Publish");
        let mut data = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::ExactlyOnce(self.trace_publish.packet_id),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
            self.trace_publish.protocol,
        );
        let mut rx_ack = self.tx.broadcast_tx.tx_pub_rec.subscribe();
        complete_to_tx_packet(
            &mut rx_ack,
            self.trace_publish.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut data,
        )
        .await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::RecoverId(self.trace_publish.packet_id))
            .await?;
        Ok(())
    }
}
