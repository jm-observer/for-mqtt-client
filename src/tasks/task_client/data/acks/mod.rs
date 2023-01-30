use crate::datas::id::Id;
use crate::tasks::task_client::data::TraceUnubscribe;
use crate::v3_1_1::SubscribeReasonCode;
use crate::QoS;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SubscribeAck {
    pub id: Id,
    pub filter_ack: Vec<SubscribeFilterAck>,
}

#[derive(Debug, Clone)]
pub struct SubscribeFilterAck {
    pub path: Arc<String>,
    pub ack: SubscribeReasonCode,
}

#[derive(Debug, Clone)]
pub struct UnsubscribeAck {
    pub id: Id,
}

impl From<TraceUnubscribe> for UnsubscribeAck {
    fn from(val: TraceUnubscribe) -> Self {
        Self { id: val.id }
    }
}
