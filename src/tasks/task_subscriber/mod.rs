mod data;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::Senders;
use crate::v3_1_1::Subscribe;
use crate::QoS;
pub use data::*;
use log::debug;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::oneshot;

/// consider the order in which pushlish   are repeated
pub struct TaskSubscriber {
    tx: Senders,
    topic: Arc<String>,
    qos: QoS,
}

impl TaskSubscriber {
    pub fn init<T: Into<Arc<String>>>(tx: Senders, topic: T, qos: QoS) {
        let topic = topic.into();
        spawn(async move {
            let mut subscriber = Self { tx, topic, qos };
            subscriber.run().await;
            //to get pkid
            //send to broker
            //wait for ack
        });
    }
    async fn run(&mut self) {
        debug!("start to subscribe");
        let (tx_pkid, rx_pkid) = oneshot::channel();
        self.tx
            .tx_hub
            .send(HubMsg::RequestId(tx_pkid))
            .await
            .unwrap();
        let pkid = rx_pkid.await.unwrap();
        debug!("pkid={}", pkid);
        let subscribe =
            Arc::new(Subscribe::new(self.topic.clone(), self.qos.clone(), pkid).unwrap());
        let mut rx_ack = self.tx.tx_subscriber.subscribe();
        let rx = self.tx.tx_network_default(subscribe.clone()).await.unwrap();
        rx.await.unwrap();
        debug!("had send to broker");
        while let Ok(msg) = rx_ack.recv().await {
            match msg {
                SubscribeMsg::SubAck(ack) => {
                    if ack.pkid == pkid {
                        debug!("{:?}", ack);
                        self.tx.tx_hub.send(HubMsg::RecoverId(pkid)).await.unwrap();
                        break;
                    }
                }
                SubscribeMsg::UnsubAck(_) => {}
            }
        }
    }
}
