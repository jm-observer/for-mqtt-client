use crate::protocol::packet::PubAck;
use crate::protocol::Protocol;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::CommonErr;
use crate::tasks::Senders;
use anyhow::Result;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos1 {
    tx: Senders,
    packet_id: u16,
    protocol: Protocol,
}

impl TaskPublishRxQos1 {
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
    async fn run(&mut self) -> Result<(), CommonErr> {
        let data = PubAck::data(self.packet_id, self.protocol);
        self.tx.tx_network_default(data).await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::AffirmRxPublish(self.packet_id))
            .await?;
        self.tx
            .tx_hub_msg
            .send(HubMsg::AffirmRxId(self.packet_id))
            .await?;
        Ok(())
    }
}
