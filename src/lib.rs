#![allow(dead_code, unused_mut, unused_imports, unused_variables)]
extern crate core;

pub mod datas;
mod tasks;
pub mod traits;
pub mod utils;
pub mod v3_1_1;

pub use tasks::MqttEvent;

#[derive(Clone)]
pub enum Transport {
    Tcp,
}

/// Quality of service
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum QoS {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
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
