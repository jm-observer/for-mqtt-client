use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::TaskSubscribe;
use crate::tasks::{MqttEvent, Senders};
use crate::QoS;
use bytes::Bytes;
use tokio::sync::broadcast::Receiver;

mod data;

pub struct Client {
    tx: Senders,
}

impl Client {
    pub fn init(tx: Senders) -> Self {
        Self { tx }
    }
    pub async fn publish(&self, topic: String, qos: QoS, payload: Bytes, retain: bool) {
        self.tx
            .tx_hub
            .send(HubMsg::Publish {
                topic,
                qos,
                payload,
                retain,
            })
            .await
            .unwrap();
    }
    pub async fn subscribe(&self, topic: String, qos: QoS) {
        self.tx
            .tx_hub
            .send(HubMsg::Subscribe { topic, qos })
            .await
            .unwrap();
    }
    pub async fn unsubscribe(&self, topic: String) {}
    pub async fn disconnect(&self) {
        self.tx.tx_hub.send(HubMsg::Disconnect).await.unwrap();
    }
    pub fn init_receiver(&self) -> Receiver<MqttEvent> {
        self.tx.rx_user()
    }
}
