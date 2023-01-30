use crate::datas::id::Id;
use crate::datas::payload::Payload;
use crate::v3_1_1::SubscribeFilter;
use crate::QoS;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TracePublish {
    id: Id,
    pub topic: Arc<String>,
    pub qos: QoS,
    pub payload: Arc<Payload>,
    pub retain: bool,
}

impl TracePublish {
    pub fn new(topic: Arc<String>, qos: QoS, payload: Arc<Payload>, retain: bool) -> Self {
        Self {
            id: Default::default(),
            topic,
            qos,
            payload,
            retain,
        }
    }
    pub fn id(&self) -> u32 {
        self.id.0
    }
}

impl PartialEq for TracePublish {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq<Id> for TracePublish {
    fn eq(&self, other: &Id) -> bool {
        &self.id == other
    }
}

#[derive(Debug, Clone)]
pub struct TraceSubscribe {
    id: Id,
    pub filters: Vec<SubscribeFilter>,
}

#[derive(Debug, Clone)]
pub struct TraceUnubscribe {
    id: Id,
    pub topics: Vec<Arc<String>>,
}
