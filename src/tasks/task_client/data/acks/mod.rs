use crate::protocol::packet::suback::SubscribeReasonCode;
use crate::protocol::packet::unsuback::{UnsubAck, UnsubAckReason};

#[derive(Debug, Clone)]
pub struct SubscribeAck {
    pub id: u32,
    pub acks: Vec<SubscribeReasonCode>,
}

#[derive(Debug, Clone)]
pub struct UnsubscribeAck {
    pub id: u32,
    pub acks: Vec<UnsubAckReason>,
}

impl UnsubscribeAck {
    pub fn init(ack: UnsubAck, id: u32) -> Self {
        match ack {
            UnsubAck::V4 { .. } => Self { id, acks: vec![] },
            UnsubAck::V5ReadMode { return_codes, .. } => Self {
                id,
                acks: return_codes,
            },
        }
    }
}
