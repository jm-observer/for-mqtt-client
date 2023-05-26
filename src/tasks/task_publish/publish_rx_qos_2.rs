use crate::{
    protocol::{
        packet::{PubComp, PubRec, PubRel},
        Protocol
    },
    tasks::{
        task_hub::HubMsg,
        utils::{complete_to_tx_packet, CommonErr},
        HubError, Senders, TIMEOUT_TO_COMPLETE_TX
    }
};
use for_event_bus::{EntryOfBus, IdentityOfSimple, ToWorker, Worker};
use tokio::spawn;

#[derive(Worker)]
/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos2 {
    tx:        Senders,
    rx:        IdentityOfSimple<PubRel>,
    packet_id: u16,
    protocol:  Protocol
}

impl TaskPublishRxQos2 {
    pub async fn init(
        bus: EntryOfBus,
        packet_id: u16,
        protocol: Protocol
    ) -> Result<(), HubError> {
        // let (tx, rx) = bus.login().await?;
        let rx = bus.simple_login::<Self, PubRel>().await?;
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

    async fn run(&mut self) -> anyhow::Result<(), CommonErr> {
        // let mut rx_ack =
        // self.tx.broadcast_tx.tx_pub_rel.subscribe();
        // self.rx.subscribe::<PubRel>()?;
        let mut data = PubRec::new(self.packet_id, self.protocol);
        complete_to_tx_packet::<PubRel, PubRec>(
            &mut self.rx,
            self.packet_id,
            TIMEOUT_TO_COMPLETE_TX,
            &self.tx,
            &mut data
        )
        .await?;
        self.rx
            .dispatch_event(HubMsg::AffirmRxPublish(self.packet_id))
            .await?;
        let data = PubComp::new(self.packet_id, self.protocol);
        self.tx.tx_network_default(data.data()).await?;
        self.rx
            .dispatch_event(HubMsg::AffirmRxId(self.packet_id))
            .await?;
        Ok(())
    }
}
