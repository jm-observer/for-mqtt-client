use crate::datas::id::Id;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

use crate::tasks::HubError;

use crate::protocol::packet::subscribe::Subscribe;
use crate::protocol::Protocol;
use anyhow::Result;
use bytes::Bytes;
use log::debug;
use ringbuf::Consumer;
use std::sync::Arc;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

#[derive(Debug, Clone)]
pub struct TracePublishQos<T> {
    pub(crate) protocol: Protocol,
    pub(crate) id: u32,
    pub topic: Arc<String>,
    pub packet_id: u16,
    qos: PhantomData<T>,
    pub payload: Arc<Bytes>,
    pub retain: bool,
}

// impl TracePublish {
//     pub fn new(topic: Arc<String>, qos: QoS, payload: Arc<Bytes>, retain: bool) -> Self {
//         match qos {
//             QoS::AtMostOnce => Self::QoS0(TracePublishQos {
//                 id: Id::id(),
//                 packet_id: 0,
//                 topic,
//                 qos: PhantomData,
//                 payload,
//                 retain,
//             }),
//             QoS::AtLeastOnce => Self::QoS1(TracePublishQos {
//                 id: Id::id(),
//                 packet_id: 0,
//                 topic,
//                 qos: PhantomData,
//                 payload,
//                 retain,
//             }),
//             QoS::ExactlyOnce => Self::QoS2(TracePublishQos {
//                 id: Id::id(),
//                 packet_id: 0,
//                 topic,
//                 qos: PhantomData,
//                 payload,
//                 retain,
//             }),
//         }
//     }
// pub fn id(&self) -> u32 {
//     match self {
//         TracePublish::QoS0(value) => value.id(),
//         TracePublish::QoS1(value) => value.id(),
//         TracePublish::QoS2(value) => value.id(),
//     }
// }

// pub(crate) fn set_packet_id(&mut self, packet_id: u16) -> &mut Self {
//     match self {
//         TracePublish::QoS0(value) => {
//             value.set_packet_id(packet_id);
//         }
//         TracePublish::QoS1(value) => {
//             value.set_packet_id(packet_id);
//         }
//         TracePublish::QoS2(value) => {
//             value.set_packet_id(packet_id);
//         }
//     };
//     self
// }
// pub(crate) fn packet_id(&self) -> u16 {
//     self.packet_id()
// }
// }

impl<T> TracePublishQos<T> {
    pub fn init(topic: Arc<String>, payload: Arc<Bytes>, retain: bool, protocol: Protocol) -> Self {
        Self {
            id: Id::id(),
            packet_id: 0,
            topic,
            qos: PhantomData,
            payload,
            retain,
            protocol,
        }
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub(crate) async fn set_packet_id(
        &mut self,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        self.packet_id = request_id(b).await?;
        Ok(())
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.packet_id
    }
}

impl<T> PartialEq for TracePublishQos<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> PartialEq<u32> for TracePublishQos<T> {
    fn eq(&self, other: &u32) -> bool {
        self.id == *other
    }
}

#[derive(Debug, Clone)]
pub struct TraceSubscribe {
    pub(crate) id: u32,
    pub(crate) subscribe: Subscribe,
}

impl TraceSubscribe {
    pub(crate) async fn set_packet_id(
        &mut self,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        let packet_id = request_id(b).await?;
        self.subscribe.set_packet_id(packet_id);
        Ok(())
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.subscribe.packet_id()
    }
}

#[derive(Debug, Clone)]
pub struct TraceUnubscribe {
    pub(crate) id: u32,
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
    pub(crate) async fn set_packet_id(
        &mut self,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        self.packet_id = request_id(b).await?;
        Ok(())
    }
    pub(crate) fn packet_id(&self) -> u16 {
        self.packet_id
    }
}

async fn request_id(b: &mut Consumer<u16, Arc<SharedRb>>) -> Result<u16, HubError> {
    if let Some(id) = b.pop() {
        debug!("request id: {}", id);
        Ok(id)
    } else {
        Err(HubError::PacketIdErr(format!(
            "buffer of packet id is empty"
        )))
    }
}
