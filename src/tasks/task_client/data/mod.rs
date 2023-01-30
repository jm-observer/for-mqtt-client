mod acks;
mod traces;

pub use acks::*;
pub use traces::*;

use crate::v3_1_1::{Publish, SubscribeFilter};
use crate::QoS;
use acks::SubscribeAck;
use bytes::Bytes;
use std::sync::Arc;

// impl<T: Into<Arc<String>>> From<(T, QoS)> for SubscribeFilter {
//     fn from(_: T) -> Self {
//         todo!()
//     }
// }

#[derive(Debug, Clone)]
pub enum MqttEvent {
    ConnectSuccess,
    ConnectFail(String),
    Publish(Publish),
    PublishSuccess(u32),
    SubscribeAck(SubscribeAck),
    UnsubscribeAck(UnsubscribeAck),
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
