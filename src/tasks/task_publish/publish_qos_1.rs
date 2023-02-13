use crate::datas::payload::Payload;
use crate::tasks::task_client::data::TracePublish;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::utils::{complete_to_tx_packet, timeout_rx};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::traits::packet_rel::PacketRel;
use crate::v3_1_1::Publish;
use crate::{QoS, QoSWithPacketId};
use anyhow::{bail, Result};
use bytes::{Bytes, BytesMut};
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, sleep_until, timeout, Instant};
use tokio::{select, spawn};

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos1 {
    tx: Senders,
    trace_publish: TracePublish,
    packet_id: u16,
}

impl TaskPublishQos1 {
    pub async fn init(tx: Senders, trace_publish: TracePublish, packet_id: u16) {
        spawn(async move {
            let mut publish = Self {
                tx,
                trace_publish,
                packet_id,
            };
            publish.run().await.unwrap();
        });
    }
    async fn run(&mut self) -> Result<()> {
        debug!("start to Publish");
        let mut packet = Publish::new(
            self.trace_publish.topic.clone(),
            QoSWithPacketId::AtLeastOnce(self.packet_id),
            self.trace_publish.payload.clone(),
            self.trace_publish.retain,
        );
        let mut rx_ack = self.tx.broadcast_tx.tx_pub_ack.subscribe();
        complete_to_tx_packet(
            &mut rx_ack,
            self.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut packet,
        )
        .await?;
        debug!("publish qos 1 success");
        self.tx
            .tx_hub_msg
            .send(HubMsg::RecoverId(self.packet_id))
            .await?;
        self.tx.tx_to_user(self.trace_publish.id());
        Ok(())
    }
}
