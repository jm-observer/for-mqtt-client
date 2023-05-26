use crate::tasks::{
    task_hub::HubMsg,
    utils::{complete_to_tx_packet, CommonErr},
    HubError, Senders, TIMEOUT_TO_COMPLETE_TX
};

use crate::protocol::{
    packet::{PubComp, PubRel},
    Protocol
};
use anyhow::Result;
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use log::debug;
use tokio::spawn;

#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos2Rel {
    protocol:  Protocol,
    tx:        Senders,
    rx:        IdentityOfSimple<PubComp>,
    packet_id: u16,
    id:        u32
}

impl TaskPublishQos2Rel {
    pub async fn init(
        bus: EntryOfBus,
        packet_id: u16,
        id: u32,
        protocol: Protocol
    ) -> Result<(), HubError> {
        let rx = bus.simple_login::<Self, PubComp>().await?;
        let tx = rx.tx();
        // let (tx, rx) = bus.login().await?;
        spawn(async move {
            let mut publish = Self {
                tx: Senders::init(tx),
                packet_id,
                rx,
                id,
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
        debug!("start to send PubRel");
        let mut data = PubRel::new(self.packet_id, self.protocol);
        // let mut rx_ack =
        // self.tx.broadcast_tx.tx_pub_comp.subscribe();
        // self.rx.subscribe::<PubComp>()?;
        complete_to_tx_packet::<PubComp, PubRel>(
            &mut self.rx,
            self.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut data
        )
        .await?;

        self.rx
            .dispatch_event(HubMsg::RecoverId(self.packet_id))
            .await?;

        self.tx.tx_to_user(self.id).await;
        Ok(())
    }
}
