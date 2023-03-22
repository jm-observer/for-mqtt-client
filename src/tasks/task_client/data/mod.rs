mod acks;
mod builder;
mod traces;

pub use acks::*;
pub use builder::*;
pub use traces::*;

use crate::protocol::packet::Publish;
use crate::protocol::Protocol;
use crate::tasks::task_network::ToConnectError;
use crate::{AtLeastOnce, AtMostOnce, ExactlyOnce, QoS};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Clone)]
pub enum ClientCommand {
    /// to send disconnect packet and drop resouces, mqtt client will diconnect event if auto reconnect
    DisconnectAndDrop,
    /// not to send disconnect packet and drop resouces, mqtt client will diconnect event if auto reconnect
    ViolenceDisconnectAndDrop,
}
#[derive(Debug, Clone)]
pub enum ClientData {
    PublishQoS0(TracePublishQos<AtMostOnce>),
    PublishQoS1(TracePublishQos<AtLeastOnce>),
    PublishQoS2(TracePublishQos<ExactlyOnce>),
    Subscribe(TraceSubscribe),
    Unsubscribe(TraceUnubscribe),
}

#[derive(Debug, Clone)]
pub enum MqttEvent {
    // session_present
    ConnectSuccess(bool),
    ConnectFail(ToConnectError),
    Publish(Publish),
    PublishSuccess(u32),
    PublishFail(String),
    SubscribeAck(SubscribeAck),
    SubscribeFail(String),
    UnsubscribeAck(UnsubscribeAck),
    UnsubscribeFail(String),
    ConnectedErr(String),
    Disconnected,
}
impl From<SubscribeAck> for MqttEvent {
    fn from(msg: SubscribeAck) -> Self {
        MqttEvent::SubscribeAck(msg)
    }
}
impl From<UnsubscribeAck> for MqttEvent {
    fn from(msg: UnsubscribeAck) -> Self {
        MqttEvent::UnsubscribeAck(msg)
    }
}
impl From<Publish> for MqttEvent {
    fn from(msg: Publish) -> Self {
        MqttEvent::Publish(msg)
    }
}

impl From<u32> for MqttEvent {
    fn from(id: u32) -> Self {
        MqttEvent::PublishSuccess(id)
    }
}

impl From<TraceUnubscribe> for ClientData {
    fn from(data: TraceUnubscribe) -> Self {
        ClientData::Unsubscribe(data)
    }
}
impl From<TraceSubscribe> for ClientData {
    fn from(data: TraceSubscribe) -> Self {
        ClientData::Subscribe(data)
    }
}

impl ClientData {
    pub fn id(&self) -> u32 {
        match self {
            ClientData::PublishQoS0(packet) => packet.id,
            ClientData::PublishQoS1(packet) => packet.id,
            ClientData::PublishQoS2(packet) => packet.id,
            ClientData::Subscribe(packet) => packet.id,
            ClientData::Unsubscribe(packet) => packet.id,
        }
    }

    pub fn publish(
        topic: Arc<String>,
        qos: QoS,
        payload: Arc<Bytes>,
        retain: bool,
        protocol: Protocol,
        id: u32,
    ) -> Self {
        match qos {
            QoS::AtMostOnce => {
                Self::PublishQoS0(TracePublishQos::init(topic, payload, retain, protocol, id))
            }
            QoS::AtLeastOnce => {
                Self::PublishQoS1(TracePublishQos::init(topic, payload, retain, protocol, id))
            }
            QoS::ExactlyOnce => {
                Self::PublishQoS2(TracePublishQos::init(topic, payload, retain, protocol, id))
            }
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum ClientErr {
    #[error("Disconnected")]
    Disconnected,
    #[error("PayloadTooLong")]
    PayloadTooLong,
}
impl<T> From<broadcast::error::SendError<T>> for ClientErr {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::Disconnected
    }
}
impl<T> From<mpsc::error::SendError<T>> for ClientErr {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::Disconnected
    }
}
