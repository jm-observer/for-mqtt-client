
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use crate::v3_1_1::{PubRel};

use anyhow::Result;
use log::debug;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos2Rel {
    tx: Senders,
    packet_id: u16,
    id: u32,
}

impl TaskPublishQos2Rel {
    pub async fn init(tx: Senders, packet_id: u16, id: u32) {
        spawn(async move {
            let mut publish = Self { tx, packet_id, id };
            if let Err(e) = publish.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> Result<(), CommonErr> {
        debug!("start to Publish");
        let mut data = PubRel::new(self.packet_id);
        let mut rx_ack = self.tx.broadcast_tx.tx_pub_comp.subscribe();
        complete_to_tx_packet(
            &mut rx_ack,
            self.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut data,
        )
        .await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::RecoverId(self.packet_id))
            .await?;
        self.tx.tx_to_user(self.id);
        Ok(())
    }
}
