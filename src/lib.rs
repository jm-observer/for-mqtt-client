// #![allow(dead_code, unused_mut, unused_imports, unused_variables)]
pub mod datas;
pub mod protocol;
mod tasks;
pub mod traits;
pub mod utils;
// pub mod v3_1_1;
// pub mod v5;

use protocol::PacketParseError;
pub use tasks::task_client::{data::*, Client};

/// Quality of service
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum QoS {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}

#[derive(Debug, Clone)]
pub struct AtMostOnce;

#[derive(Debug, Clone)]
pub struct AtLeastOnce;

#[derive(Debug, Clone)]
pub struct ExactlyOnce;

#[derive(Debug, Clone)]
pub struct ProtocolV4;

#[derive(Debug, Clone)]
pub struct ProtocolV5;

pub trait Protocol {
    fn is_v4() -> bool {
        true
    }
    fn is_v5() -> bool {
        false
    }
}

impl Protocol for ProtocolV4 {}
impl Protocol for ProtocolV5 {
    fn is_v5() -> bool {
        true
    }
    fn is_v4() -> bool {
        false
    }
}

/// Quality of service
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum QoSWithPacketId {
    AtMostOnce,
    AtLeastOnce(u16),
    ExactlyOnce(u16),
}

impl QoSWithPacketId {
    pub fn qos(&self) -> u8 {
        match self {
            QoSWithPacketId::AtMostOnce => 0,
            QoSWithPacketId::AtLeastOnce(_) => 1,
            QoSWithPacketId::ExactlyOnce(_) => 2,
        }
    }
    pub fn packet_id(&self) -> Option<u16> {
        match self {
            QoSWithPacketId::AtMostOnce => None,
            QoSWithPacketId::AtLeastOnce(packet_id) => Some(packet_id.clone()),
            QoSWithPacketId::ExactlyOnce(packet_id) => Some(packet_id.clone()),
        }
    }
}

/// Maps a number to QoS
pub fn qos(num: u8) -> Result<QoS, PacketParseError> {
    match num {
        0 => Ok(QoS::AtMostOnce),
        1 => Ok(QoS::AtLeastOnce),
        2 => Ok(QoS::ExactlyOnce),
        qos => Err(PacketParseError::InvalidQoS(qos)),
    }
}
