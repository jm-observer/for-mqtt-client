use crate::datas::id::Id;
use crate::datas::payload::Payload;
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
