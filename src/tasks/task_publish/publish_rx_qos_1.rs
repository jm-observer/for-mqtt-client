use crate::{
    protocol::{packet::PubAck, Protocol},
    tasks::{task_hub::HubMsg, utils::CommonErr, HubError, Senders}
};
use anyhow::Result;
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use tokio::spawn;

#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos1 {
    tx:        Senders,
    packet_id: u16,
    rx:        IdentityOfSimple<()>,
    protocol:  Protocol
}

impl TaskPublishRxQos1 {
    pub async fn init(
        bus: EntryOfBus,
        packet_id: u16,
        protocol: Protocol
    ) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login::<Self, ()>().await?;
        let tx = rx.tx();
        spawn(async move {
            let mut publish = Self {
                tx: Senders::init(tx),
                rx,
                packet_id,
                protocol
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
        let data = PubAck::new(self.packet_id, self.protocol);
        self.tx.tx_network_default(data.data()).await?;
        self.rx
            .dispatch_event(HubMsg::AffirmRxPublish(self.packet_id))
            .await?;
        self.rx
            .dispatch_event(HubMsg::AffirmRxId(self.packet_id))
            .await?;
        Ok(())
    }
}
