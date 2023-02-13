mod acks;
mod traces;

pub use acks::*;
pub use traces::*;

use crate::v3_1_1::{Publish, SubscribeFilter};
use crate::QoS;
use bytes::Bytes;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ClientCommand {
    Connect,
    Disconnect,
    Publish(TracePublish),
    Subscribe(TraceSubscribe),
    Unsubscribe(TraceUnubscribe),
}

#[derive(Debug, Clone)]
pub enum MqttEvent {
    ConnectSuccess,
    ConnectFail(String),
    Publish(Publish),
    PublishSuccess(u32),
    SubscribeAck(SubscribeAck),
    UnsubscribeAck(UnsubscribeAck),
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
