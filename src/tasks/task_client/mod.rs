use crate::datas::payload::Payload;
use crate::tasks::task_client::data::{TraceSubscribe, TraceUnubscribe};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::TaskSubscribe;
use crate::tasks::Senders;
use crate::v3_1_1::{Error, SubscribeFilter};
use crate::QoS;
use bytes::Bytes;
use data::MqttEvent;
use data::TracePublish;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;

pub mod data;

#[derive(Clone)]
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
    pub async fn subscribe<T: Into<Arc<String>>>(&self, topic: T, qos: QoS) -> TraceSubscribe {
        let filter = SubscribeFilter::new(topic, qos);
        let trace = TraceSubscribe::new(vec![filter]);
        self.tx
            .tx_hub
            .send(HubMsg::Subscribe(trace.clone()))
            .await
            .unwrap();
        trace
    }
    pub async fn unsubscribe(&self, topic: String) -> TraceUnubscribe {
        let topic = Arc::new(topic);
        let trace = TraceUnubscribe::new(vec![topic]);
        self.tx
            .tx_hub
            .send(HubMsg::Unsubscribe(trace.clone()))
            .await
            .unwrap();
        trace
    }
    pub async fn disconnect(&self) {
        self.tx.tx_hub.send(HubMsg::Disconnect).await.unwrap();
    }
    pub fn init_receiver(&self) -> Receiver<MqttEvent> {
        self.tx.rx_user()
    }
}
