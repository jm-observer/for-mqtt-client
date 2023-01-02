use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{PubRel, Publish};
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::broadcast::error::RecvError;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos2 {
    tx: Senders,
    topic: Arc<String>,
    payload: Bytes,
    qos: QoS,
    retain: bool,
    pkid: u16,
}

impl TaskPublishQos2 {
    pub async fn init(
        tx: Senders,
        topic: Arc<String>,
        payload: Bytes,
        qos: QoS,
        retain: bool,
        pkid: u16,
    ) {
        spawn(async move {
            let mut publish = Self {
                tx,
                topic,
                payload,
                qos,
                retain,
                pkid,
            };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        debug!("start to Publish");
        let packet = Publish::new(
            self.topic.clone(),
            self.qos.clone(),
            self.payload.clone(),
            self.retain,
            Some(self.pkid),
        );
        let mut bytes = BytesMut::new();
        let mut rx_ack = self.tx.tx_publish.subscribe();
        packet.write(&mut bytes).unwrap();
        let data = bytes.freeze();
        let rx = self.tx.tx_network_default(data).await.unwrap();
        rx.await.unwrap();
        let mut state = StateQos2::WaitRec;
        loop {
            match rx_ack.recv().await {
                Ok(ack) => match state {
                    StateQos2::WaitRec => match ack {
                        PublishMsg::PubRec(ack) => {
                            if ack.pkid == self.pkid {
                                debug!("wait for comp...");
                                state = StateQos2::WaitComp;
                                let data = PubRel::new(self.pkid).unwrap();
                                let rx = self.tx.tx_network_default(data).await.unwrap();
                                rx.await.unwrap();
                            }
                        }
                        _ => {}
                    },
                    StateQos2::WaitComp => match ack {
                        PublishMsg::PubComp(ack) => {
                            if ack.pkid == self.pkid {
                                debug!("publish qos 2 success");
                                self.tx
                                    .tx_hub
                                    .send(HubMsg::RecoverId(self.pkid))
                                    .await
                                    .unwrap();
                                return;
                            }
                        }
                        _ => {}
                    },
                },
                Err(e) => {}
            }
        }
    }
}

enum StateQos2 {
    WaitRec,
    WaitComp,
}
