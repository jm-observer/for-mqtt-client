use crate::datas::payload::Payload;
use crate::tasks::task_client::data::{TraceSubscribe, TraceUnubscribe};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_subscribe::TaskSubscribe;
use crate::tasks::{BroadcastTx, Senders};
use crate::v3_1_1::{Error, SubscribeFilter};
use crate::{ClientCommand, ClientData, QoS};
use bytes::Bytes;
use data::MqttEvent;
use data::TracePublish;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc;

pub mod data;

#[derive(Clone)]
pub struct Client {
    tx_client_command: mpsc::Sender<ClientCommand>,
    tx_client_data: mpsc::Sender<ClientData>,
    tx_to_client: Sender<MqttEvent>,
}

impl Client {
    pub fn init(
        tx_client_data: mpsc::Sender<ClientData>,
        tx_client_command: mpsc::Sender<ClientCommand>,
        tx_to_client: Sender<MqttEvent>,
    ) -> Self {
        Self {
            tx_client_command,
            tx_to_client,
            tx_client_data,
        }
    }
    pub async fn publish<T: Into<Arc<String>>, D: Into<Bytes>>(
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
        let trace_publish = TracePublish::new(topic, qos, payload.into(), retain);
        self.tx_client_data
            .send(ClientData::Publish(trace_publish.clone()))
            .await
            .unwrap();
        Ok(trace_publish)
    }
    pub async fn publish_by_arc<T: Into<Arc<String>>>(
        &self,
        topic: T,
        qos: QoS,
        payload: Arc<Bytes>,
        retain: bool,
    ) -> Result<TracePublish, Error> {
        let topic = topic.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(Error::PayloadTooLong);
        };
        let trace_publish = TracePublish::new(topic, qos, payload, retain);
        self.tx_client_data
            .send(ClientData::Publish(trace_publish.clone()))
            .await
            .unwrap();
        Ok(trace_publish)
    }
    pub async fn subscribe<T: Into<Arc<String>>>(&self, topic: T, qos: QoS) -> TraceSubscribe {
        let filter = SubscribeFilter::new(topic, qos);
        let trace = TraceSubscribe::new(vec![filter]);
        self.tx_client_data
            .send(ClientData::Subscribe(trace.clone()))
            .await
            .unwrap();
        trace
    }
    pub async fn unsubscribe(&self, topic: String) -> TraceUnubscribe {
        let topic = Arc::new(topic);
        let trace = TraceUnubscribe::new(vec![topic]);
        self.tx_client_data
            .send(ClientData::Unsubscribe(trace.clone()))
            .await
            .unwrap();
        trace
    }
    pub async fn disconnect(&self) {
        self.tx_client_command
            .send(ClientCommand::DisconnectAndDrop)
            .await
            .unwrap();
    }
    pub fn init_receiver(&self) -> Receiver<MqttEvent> {
        self.tx_to_client.subscribe()
    }
}
