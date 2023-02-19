use crate::datas::id::Id;

use crate::v3_1_1::SubscribeFilter;
use crate::QoS;
use bytes::Bytes;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TracePublish {
    id: u32,
    packet_id: u16,
    pub topic: Arc<String>,
    pub qos: QoS,
    pub payload: Arc<Bytes>,
    pub retain: bool,
}

impl TracePublish {
    pub fn new(topic: Arc<String>, qos: QoS, payload: Arc<Bytes>, retain: bool) -> Self {
        Self {
            id: Id::id(),
            packet_id: 0,
            topic,
            qos,
            payload,
            retain,
        }
    }
    pub fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn set_packet_id(&mut self, packet_id: u16) -> &mut Self {
        self.packet_id = packet_id;
        self
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.packet_id
    }
}

impl PartialEq for TracePublish {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialEq<u32> for TracePublish {
    fn eq(&self, other: &u32) -> bool {
        self.id == *other
    }
}

#[derive(Debug, Clone)]
pub struct TraceSubscribe {
    pub id: u32,
    pub(crate) packet_id: u16,
    pub filters: Vec<SubscribeFilter>,
}

impl TraceSubscribe {
    pub fn new(filters: Vec<SubscribeFilter>) -> Self {
        Self {
            id: Id::id(),
            packet_id: 0,
            filters,
        }
    }
    pub(crate) fn set_packet_id(&mut self, packet_id: u16) -> &mut Self {
        self.packet_id = packet_id;
        self
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.packet_id
    }
}

#[derive(Debug, Clone)]
pub struct TraceUnubscribe {
    pub id: u32,
    pub(crate) packet_id: u16,
    pub topics: Vec<Arc<String>>,
}
impl TraceUnubscribe {
    pub fn new(topics: Vec<Arc<String>>) -> Self {
        Self {
            id: Id::id(),
            packet_id: 0,
            topics,
        }
    }
    pub(crate) fn set_packet_id(&mut self, packet_id: u16) -> &mut Self {
        self.packet_id = packet_id;
        self
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.packet_id
    }
}
