use crate::tasks::task_client::data::{TraceSubscribe, TraceUnubscribe};

use crate::protocol::Protocol;
use crate::v3_1_1::SubscribeFilter;
use crate::{ClientCommand, ClientData, ClientErr, QoS};
use bytes::Bytes;
use data::MqttEvent;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::mpsc;

pub mod data;

#[derive(Clone)]
pub struct Client {
    pub(crate) protocol: Protocol,
    tx_client_command: mpsc::Sender<ClientCommand>,
    tx_client_data: mpsc::Sender<ClientData>,
    tx_to_client: Sender<MqttEvent>,
}

impl Client {
    pub fn init(
        tx_client_data: mpsc::Sender<ClientData>,
        tx_client_command: mpsc::Sender<ClientCommand>,
        tx_to_client: Sender<MqttEvent>,
        protocol: Protocol,
    ) -> Self {
        Self {
            tx_client_command,
            tx_to_client,
            tx_client_data,
            protocol,
        }
    }
    pub async fn publish<T: Into<Arc<String>>, D: Into<Bytes>>(
        &self,
        topic: T,
        qos: QoS,
        payload: D,
        retain: bool,
    ) -> Result<u32, ClientErr> {
        let topic = topic.into();
        let payload = payload.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = ClientData::publish(topic, qos, payload.into(), retain, self.protocol);
        let id = trace_publish.id();
        self.tx_client_data.send(trace_publish).await?;
        Ok(id)
    }
    pub async fn publish_by_arc<T: Into<Arc<String>>>(
        &self,
        topic: T,
        qos: QoS,
        payload: Arc<Bytes>,
        retain: bool,
    ) -> Result<u32, ClientErr> {
        let topic = topic.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = ClientData::publish(topic, qos, payload, retain, self.protocol);
        let id = trace_publish.id();
        self.tx_client_data.send(trace_publish).await?;
        Ok(id)
    }
    pub async fn subscribe<T: Into<Arc<String>>>(&self, topic: T, qos: QoS) -> anyhow::Result<u32> {
        let filter = SubscribeFilter::new(topic, qos);
        let trace = TraceSubscribe::new(vec![filter]);
        let id = trace.id;
        self.tx_client_data
            .send(ClientData::Subscribe(trace))
            .await?;
        Ok(id)
    }
    pub async fn unsubscribe(&self, topic: String) -> anyhow::Result<u32> {
        let topic = Arc::new(topic);
        let trace = TraceUnubscribe::new(vec![topic]);
        let id = trace.id;
        self.tx_client_data
            .send(ClientData::Unsubscribe(trace))
            .await?;
        Ok(id)
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
