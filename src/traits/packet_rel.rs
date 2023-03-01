use crate::protocol::packet::{PubAck, PubComp, PubRec, PubRel};
use crate::v3_1_1::{SubAck, UnsubAck};
use std::fmt::Debug;

pub trait PacketRel: Clone + Debug {
    fn is_rel(&self, packet_id: u16) -> bool;
}

impl PacketRel for PubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}

impl PacketRel for PubRel {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}

impl PacketRel for PubRec {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}
impl PacketRel for PubComp {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}
impl PacketRel for SubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}
impl PacketRel for UnsubAck {
    fn is_rel(&self, packet_id: u16) -> bool {
        self.packet_id == packet_id
    }
}
