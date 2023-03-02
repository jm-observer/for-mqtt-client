use crate::protocol::packet::{PubComp, PubRec};
use crate::protocol::Protocol;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::{complete_to_tx_packet, CommonErr};
use crate::tasks::{Senders, TIMEOUT_TO_COMPLETE_TX};
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos2 {
    tx: Senders,
    packet_id: u16,
    protocol: Protocol,
}

impl TaskPublishRxQos2 {
    pub fn init(tx: Senders, packet_id: u16, protocol: Protocol) {
        spawn(async move {
            let mut publish = Self {
                tx,
                packet_id,
                protocol,
            };
            if let Err(e) = publish.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> anyhow::Result<(), CommonErr> {
        let mut rx_ack = self.tx.broadcast_tx.tx_pub_rel.subscribe();
        let mut data = PubRec::new(self.packet_id, self.protocol);
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
            .send(HubMsg::AffirmRxPublish(self.packet_id))
            .await?;
        let data = PubComp::new(self.packet_id, self.protocol);
        self.tx.tx_network_default(data.data()).await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::AffirmRxId(self.packet_id))
            .await?;
        Ok(())
    }
}
