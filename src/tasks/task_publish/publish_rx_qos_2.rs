use crate::tasks::task_publish::PublishMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{PubAck, PubComp, PubRec, Publish};
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::{debug, error};
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishRxQos2 {
    tx: Senders,
    topic: Arc<String>,
    payload: Bytes,
    qos: QoS,
    pkid: u16,
}

impl TaskPublishRxQos2 {
    pub async fn init(tx: Senders, topic: Arc<String>, payload: Bytes, qos: QoS, pkid: u16) {
        spawn(async move {
            let mut publish = Self {
                tx,
                topic,
                payload,
                qos,
                pkid,
            };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        let mut rx_ack = self.tx.tx_publish.subscribe();
        let data = Arc::new(PubRec::new(self.pkid).unwrap());
        let rx = self.tx.tx_network_default(data).await.unwrap();
        rx.await.unwrap();

        loop {
            match rx_ack.recv().await {
                Ok(ack) => match ack {
                    PublishMsg::PubRel(ack) => {
                        if ack.pkid == self.pkid {
                            let data = PubComp::new(self.pkid).unwrap();
                            self.tx.tx_network_without_receipt(data).await.unwrap();
                            // todo send publish to hub or user
                        }
                    }
                    _ => {
                        // todo conside dup
                    }
                },
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }

        loop {
            match rx_ack.recv().await {
                Ok(ack) => match ack {
                    PublishMsg::PubRel(ack) => {
                        if ack.pkid == self.pkid {
                            let data = PubComp::new(self.pkid).unwrap();
                            self.tx.tx_network_without_receipt(data).await.unwrap();
                            // todo send publish to hub or user
                        }
                    }
                    _ => {
                        // todo conside dup
                    }
                },
                Err(e) => {
                    error!("{:?}", e);
                }
            }
        }
    }
}
