use crate::protocol::Protocol;
use crate::tasks::task_publish::{TaskPublishQos1, TaskPublishQos2, TaskPublishQos2Rel};
use crate::tasks::task_subscribe::TaskUnsubscribe;
use crate::tasks::{HubError, TaskSubscribe};
use crate::{AtLeastOnce, ExactlyOnce, TracePublishQos, TraceSubscribe, TraceUnubscribe};
use for_event_bus::CopyOfBus;
use std::mem;

pub enum UnacknowledgedClientData {
    PublishQoS1(TracePublishQos<AtLeastOnce>),
    PublishQoS2(TracePublishQos<ExactlyOnce>),
    /// packet_id, trace_id
    PubRel(u16, u32, Protocol),
    Subscribe(TraceSubscribe),
    Unsubscribe(TraceUnubscribe),
}

impl UnacknowledgedClientData {
    pub fn packet_id(&self) -> u16 {
        match self {
            UnacknowledgedClientData::PubRel(packet_id, ..) => *packet_id,
            UnacknowledgedClientData::Subscribe(packet) => packet.packet_id(),
            UnacknowledgedClientData::Unsubscribe(packet) => packet.packet_id(),
            UnacknowledgedClientData::PublishQoS1(packet) => packet.packet_id(),
            UnacknowledgedClientData::PublishQoS2(packet) => packet.packet_id(),
        }
    }
    ///
    /// return is_completed
    pub fn acknowledge(&mut self) -> bool {
        match &*self {
            UnacknowledgedClientData::PublishQoS2(packet) => {
                let _ = mem::replace(
                    self,
                    UnacknowledgedClientData::PubRel(
                        packet.packet_id(),
                        packet.id(),
                        packet.protocol,
                    ),
                );
                false
            }
            UnacknowledgedClientData::PubRel(..)
            | UnacknowledgedClientData::Subscribe(_)
            | UnacknowledgedClientData::Unsubscribe(_)
            | UnacknowledgedClientData::PublishQoS1(_) => true,
        }
    }

    pub async fn to_acknowledge(&self, senders: &CopyOfBus) -> Result<(), HubError> {
        match self {
            UnacknowledgedClientData::PublishQoS1(packet) => {
                TaskPublishQos1::init(senders.clone(), packet.clone()).await?;
            }
            UnacknowledgedClientData::PublishQoS2(packet) => {
                TaskPublishQos2::init(senders.clone(), packet.clone()).await?;
            }
            UnacknowledgedClientData::PubRel(packet_id, id, protocol) => {
                TaskPublishQos2Rel::init(senders.clone(), *packet_id, *id, *protocol).await?
            }
            UnacknowledgedClientData::Subscribe(packet) => {
                TaskSubscribe::init(senders.clone(), packet.clone()).await?;
            }
            UnacknowledgedClientData::Unsubscribe(packet) => {
                TaskUnsubscribe::init(senders.clone(), packet.clone()).await?;
            }
        }
        Ok(())
    }
}

impl From<TracePublishQos<AtLeastOnce>> for UnacknowledgedClientData {
    fn from(value: TracePublishQos<AtLeastOnce>) -> Self {
        UnacknowledgedClientData::PublishQoS1(value)
    }
}
impl From<TracePublishQos<ExactlyOnce>> for UnacknowledgedClientData {
    fn from(value: TracePublishQos<ExactlyOnce>) -> Self {
        UnacknowledgedClientData::PublishQoS2(value)
    }
}
impl From<TraceUnubscribe> for UnacknowledgedClientData {
    fn from(value: TraceUnubscribe) -> Self {
        UnacknowledgedClientData::Unsubscribe(value)
    }
}
impl From<TraceSubscribe> for UnacknowledgedClientData {
    fn from(value: TraceSubscribe) -> Self {
        UnacknowledgedClientData::Subscribe(value)
    }
}
