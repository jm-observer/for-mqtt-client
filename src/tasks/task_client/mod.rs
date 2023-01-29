use crate::datas::payload::Payload;
use crate::datas::trace_publish::TracePublish;
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::TaskSubscribe;
use crate::tasks::{MqttEvent, Senders};
use crate::v3_1_1::Error;
use crate::QoS;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

mod data;

pub struct Client {
    tx: Senders,
}

impl Client {
    pub fn init(tx: Senders) -> Self {
        Self { tx }
    }
    pub async fn publish<T: Into<Arc<String>>, D: Into<Payload>>(
        &self,
        topic: T,
        qos: QoS,
        payload: D,
        retain: bool,
    ) -> Result<TracePublish, Error> {
        let topic = topic.into();
        let payload = payload.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(Error::PayloadTooLong);
        };
        let payload = Arc::new(payload);
        let trace_publish = TracePublish::new(topic, qos, payload, retain);
        self.tx
            .tx_hub
            .send(HubMsg::Publish(trace_publish.clone()))
            .await
            .unwrap();
        Ok(trace_publish)
    }
    pub async fn subscribe(&self, topic: String, qos: QoS) {
        self.tx
            .tx_hub
            .send(HubMsg::Subscribe { topic, qos })
            .await
            .unwrap();
    }
    pub async fn unsubscribe(&self, topic: String) {
        self.tx
            .tx_hub
            .send(HubMsg::Unsubscribe { topic })
            .await
            .unwrap();
    }
    pub async fn disconnect(&self) {
        self.tx.tx_hub.send(HubMsg::Disconnect).await.unwrap();
    }
    pub fn init_receiver(&self) -> Receiver<MqttEvent> {
        self.tx.rx_user()
    }
}
