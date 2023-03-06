use crate::protocol::packet::SubscribeReasonCode;
use crate::protocol::packet::{UnsubAck, UnsubAckReason};

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
