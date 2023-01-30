use crate::datas::id::Id;
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
    pub ack: SubscribeAckResult,
}

#[derive(Debug, Clone)]
pub enum SubscribeAckResult {
    Success(QoS),
    Fail,
}
#[derive(Debug, Clone)]
pub struct UnsubscribeAck {
    pub id: Id,
}
