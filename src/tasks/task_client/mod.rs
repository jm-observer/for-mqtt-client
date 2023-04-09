use crate::tasks::task_client::data::TraceSubscribe;

use crate::datas::id::Id;
use crate::protocol::Protocol;
use crate::{
    ClientCommand, ClientData, ClientErr, FilterBuilder, MqttEvent, ProtocolV4, ProtocolV5, QoS,
    TraceUnubscribe, UnsubscribeFilterBuilder,
};
use bytes::Bytes;
use for_event_bus::worker::{IdentityOfSimple, IdentityOfTx, Worker};
use for_event_bus::{BusError, CopyOfBus};
use std::sync::Arc;

pub mod data;

pub struct Client {
    protocol: Protocol,
    // bus: CopyOfBus,
    identity_tx: IdentityOfTx,
}

#[allow(dead_code)]
pub struct ClientRx {
    protocol: Protocol,
    // pub bus: CopyOfBus,
    identity: IdentityOfSimple<MqttEvent>,
}

impl ClientRx {
    pub fn identity_mut(&mut self) -> &mut IdentityOfSimple<MqttEvent> {
        &mut self.identity
    }
}

impl Worker for Client {
    fn identity_tx(&self) -> &IdentityOfTx {
        &self.identity_tx
    }
}

impl Client {
    pub(crate) async fn init(
        protocol: Protocol,
        bus: CopyOfBus,
    ) -> Result<(Client, ClientRx), BusError> {
        let identity = bus.simple_login::<MqttEvent>().await?;
        let identity_tx = identity.tx();
        Ok((
            Client {
                protocol,
                // bus: bus.clone(),
                identity_tx,
            },
            ClientRx {
                protocol,
                // bus,
                identity,
            },
        ))
    }
    pub async fn publish<T: Into<Arc<String>>, D: Into<Bytes>>(
        &self,
        topic: T,
        qos: QoS,
        payload: D,
        retain: bool,
    ) -> Result<u32, ClientErr> {
        let id = Id::id();
        self.publish_with_trace_id(topic, qos, payload, retain, id)
            .await?;
        Ok(id)
    }
    pub async fn publish_with_trace_id<T: Into<Arc<String>>, D: Into<Bytes>>(
        &self,
        topic: T,
        qos: QoS,
        payload: D,
        retain: bool,
        trace_id: u32,
    ) -> Result<(), ClientErr> {
        let topic = topic.into();
        let payload = payload.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = ClientData::publish(
            topic,
            qos,
            payload.into(),
            retain,
            self.protocol(),
            trace_id,
        );
        self.identity_tx.dispatch_event(trace_publish)?;
        Ok(())
    }
    pub async fn publish_by_arc<T: Into<Arc<String>>>(
        &self,
        topic: T,
        qos: QoS,
        payload: Arc<Bytes>,
        retain: bool,
    ) -> Result<u32, ClientErr> {
        let id = Id::id();
        let topic = topic.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(ClientErr::PayloadTooLong);
        };
        let trace_publish = ClientData::publish(topic, qos, payload, retain, self.protocol(), id);
        self.identity_tx.dispatch_event(trace_publish)?;
        Ok(id)
    }
    pub async fn to_subscribe<T: Into<String>>(
        &self,
        topic: T,
        qos: QoS,
    ) -> Result<u32, ClientErr> {
        let id = Id::id();
        self.to_subscribe_with_trace_id(topic, qos, id).await?;
        Ok(id)
    }

    pub async fn to_subscribe_with_trace_id<T: Into<String>>(
        &self,
        topic: T,
        qos: QoS,
        id: u32,
    ) -> Result<(), ClientErr> {
        let subscribe: TraceSubscribe = match self.protocol {
            Protocol::V4 => FilterBuilder::<ProtocolV4>::new(topic.into(), qos)
                .build(id)
                .into(),
            Protocol::V5 => FilterBuilder::<ProtocolV5>::new(topic.into(), qos)
                .build(id)
                .into(),
        };

        self.identity_tx
            .dispatch_event(ClientData::Subscribe(subscribe))?;
        Ok(())
    }

    pub async fn unsubscribe(&self, topic: String) -> Result<u32, ClientErr> {
        let id = Id::id();
        self.unsubscribe_with_trace_id(topic, id).await?;
        Ok(id)
    }

    pub async fn unsubscribe_with_trace_id(&self, topic: String, id: u32) -> Result<(), ClientErr> {
        let unsubscribe: TraceUnubscribe = match self.protocol {
            Protocol::V4 => UnsubscribeFilterBuilder::<ProtocolV4>::new(topic)
                .build(id)
                .into(),
            Protocol::V5 => UnsubscribeFilterBuilder::<ProtocolV5>::new(topic)
                .build(id)
                .into(),
        };

        self.identity_tx
            .dispatch_event(ClientData::Unsubscribe(unsubscribe))?;
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), ClientErr> {
        Ok(self
            .identity_tx
            .dispatch_event(ClientCommand::DisconnectAndDrop)?)
    }

    fn protocol(&self) -> Protocol {
        self.protocol
    }
}
