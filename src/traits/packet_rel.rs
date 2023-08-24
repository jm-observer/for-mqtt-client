use crate::protocol::packet::SubAck;
use crate::protocol::packet::UnsubAck;
use crate::protocol::packet::{PubAck, PubComp, PubRec, PubRel};
use for_event_bus::Event;
use std::fmt::Debug;

pub trait PacketRel:
    Event + Clone + Debug + Send + Sync + 'static
{
    fn is_rel(&self, packet_id: u16) -> bool;
}

impl PacketRel for PubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}

impl PacketRel for PubRel {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}

impl PacketRel for PubRec {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}
impl PacketRel for PubComp {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}
impl PacketRel for SubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}
impl PacketRel for UnsubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id() == packet_id
    }
}
