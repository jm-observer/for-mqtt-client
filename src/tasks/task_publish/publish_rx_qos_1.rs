use crate::protocol::packet::PubAck;
use crate::protocol::Protocol;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::CommonErr;
use crate::tasks::{HubError, Senders};
use anyhow::Result;
use for_event_bus::worker::IdentityOfSimple;
use for_event_bus::CopyOfBus;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos1 {
    tx: Senders,
    packet_id: u16,
    rx: IdentityOfSimple<()>,
    protocol: Protocol,
}

impl TaskPublishRxQos1 {
    pub async fn init(bus: CopyOfBus, packet_id: u16, protocol: Protocol) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login().await?;
        let tx = rx.tx();
        spawn(async move {
            let mut publish = Self {
                tx: Senders::init(tx),
                rx,
                packet_id,
                protocol,
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
            .dispatch_event(HubMsg::AffirmRxPublish(self.packet_id))?;
        self.rx.dispatch_event(HubMsg::AffirmRxId(self.packet_id))?;
        Ok(())
    }
}
