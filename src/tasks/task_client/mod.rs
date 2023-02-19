
use crate::tasks::task_client::data::{TraceSubscribe, TraceUnubscribe};



use crate::v3_1_1::{SubscribeFilter};
use crate::{ClientCommand, ClientData, ClientErr, QoS};
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
    ) -> Result<TracePublish, ClientErr> {
        let topic = topic.into();
        let payload = payload.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = TracePublish::new(topic, qos, payload.into(), retain);
        self.tx_client_data
            .send(ClientData::Publish(trace_publish.clone()))
            .await?;
        Ok(trace_publish)
    }
    pub async fn publish_by_arc<T: Into<Arc<String>>>(
        &self,
        topic: T,
        qos: QoS,
        payload: Arc<Bytes>,
        retain: bool,
    ) -> Result<TracePublish, ClientErr> {
        let topic = topic.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = TracePublish::new(topic, qos, payload, retain);
        self.tx_client_data
            .send(ClientData::Publish(trace_publish.clone()))
            .await?;
        Ok(trace_publish)
    }
    pub async fn subscribe<T: Into<Arc<String>>>(
        &self,
        topic: T,
        qos: QoS,
    ) -> anyhow::Result<TraceSubscribe> {
        let filter = SubscribeFilter::new(topic, qos);
        let trace = TraceSubscribe::new(vec![filter]);
        self.tx_client_data
            .send(ClientData::Subscribe(trace.clone()))
            .await?;
        Ok(trace)
    }
    pub async fn unsubscribe(&self, topic: String) -> anyhow::Result<TraceUnubscribe> {
        let topic = Arc::new(topic);
        let trace = TraceUnubscribe::new(vec![topic]);
        self.tx_client_data
            .send(ClientData::Unsubscribe(trace.clone()))
            .await?;
        Ok(trace)
    }
    pub async fn disconnect(&self) -> anyhow::Result<()> {
        self.tx_client_command
            .send(ClientCommand::DisconnectAndDrop)
            .await?;
        Ok(())
    }
    pub fn init_receiver(&self) -> Receiver<MqttEvent> {
        self.tx_to_client.subscribe()
    }
}
