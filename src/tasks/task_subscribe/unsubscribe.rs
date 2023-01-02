use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{Subscribe, Unsubscribe};
use crate::QoS;
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskUnsubscribe {
    tx: Senders,
    topic: Arc<String>,
    pkid: u16,
}

impl TaskUnsubscribe {
    pub fn init<T: Into<Arc<String>>>(tx: Senders, topic: T, pkid: u16) {
        let topic = topic.into();
        spawn(async move {
            let mut unsubscribe = Self { tx, topic, pkid };
            unsubscribe.run().await;
        });
    }
    async fn run(&mut self) {
        debug!("start to unsubscribe");
        let subscribe = Arc::new(Unsubscribe::new(self.topic.clone(), self.pkid).unwrap());
        let mut rx_ack = self.tx.tx_subscribe.subscribe();
        let rx = self.tx.tx_network_default(subscribe.clone()).await.unwrap();
        rx.await.unwrap();
        debug!("unsubscribe had send to broker");
        while let Ok(msg) = rx_ack.recv().await {
            match msg {
                SubscribeMsg::SubAck(_) => {}
                SubscribeMsg::UnsubAck(ack) => {
                    if ack.pkid == self.pkid {
                        debug!("{:?}", ack);
                        self.tx
                            .tx_hub
                            .send(HubMsg::RecoverId(self.pkid))
                            .await
                            .unwrap();
                        break;
                    }
                }
            }
        }
    }
}
