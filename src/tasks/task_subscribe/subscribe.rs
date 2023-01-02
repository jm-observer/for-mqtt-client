use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::SubscribeMsg;
use crate::tasks::Senders;
use crate::v3_1_1::Subscribe;
use crate::QoS;
use log::debug;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskSubscribe {
    tx: Senders,
    topic: Arc<String>,
    qos: QoS,
    pkid: u16,
}

impl TaskSubscribe {
    pub fn init<T: Into<Arc<String>>>(tx: Senders, topic: T, qos: QoS, pkid: u16) {
        let topic = topic.into();
        spawn(async move {
            let mut subscriber = Self {
                tx,
                topic,
                qos,
                pkid,
            };
            subscriber.run().await;
        });
    }
    async fn run(&mut self) {
        debug!("start to subscribe");
        let subscribe =
            Arc::new(Subscribe::new(self.topic.clone(), self.qos.clone(), self.pkid).unwrap());
        let mut rx_ack = self.tx.tx_subscribe.subscribe();
        let rx = self.tx.tx_network_default(subscribe.clone()).await.unwrap();
        rx.await.unwrap();
        debug!("had send to broker");
        while let Ok(msg) = rx_ack.recv().await {
            match msg {
                SubscribeMsg::SubAck(ack) => {
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
                SubscribeMsg::UnsubAck(_) => {}
            }
        }
    }
}
