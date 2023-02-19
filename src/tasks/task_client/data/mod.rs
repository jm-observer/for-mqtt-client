mod acks;
mod traces;

pub use acks::*;
pub use traces::*;

use crate::tasks::task_network::ToConnectError;
use crate::v3_1_1::{MqttOptions, Publish, SubscribeFilter};
use crate::QoS;
use bytes::Bytes;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ClientCommand {
    /// to send disconnect packet and drop resouces, mqtt client will diconnect event if auto reconnect
    DisconnectAndDrop,
    /// not to send disconnect packet and drop resouces, mqtt client will diconnect event if auto reconnect
    ViolenceDisconnectAndDrop,
}
#[derive(Debug, Clone)]
pub enum ClientData {
    Publish(TracePublish),
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

impl From<TracePublish> for ClientData {
    fn from(data: TracePublish) -> Self {
        ClientData::Publish(data)
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
    pub(crate) fn packet_id(&self) -> u16 {
        match self {
            ClientData::Publish(data) => data.packet_id(),
            ClientData::Subscribe(data) => data.packet_id(),
            ClientData::Unsubscribe(data) => data.packet_id(),
        }
    }
}
