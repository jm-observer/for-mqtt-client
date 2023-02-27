//! # mqttbytes
//!
//! This module contains the low level struct definitions required to assemble and disassemble MQTT 3.1.1 packets in rumqttc.
//! The [`bytes`](https://docs.rs/bytes) crate is used internally.

use bytes::{BufMut, Bytes, BytesMut};
use core::fmt;

// mod connack;
// mod connect;
mod disconnect;
mod ping;
mod puback;
mod pubcomp;
mod publish;
mod pubrec;
mod pubrel;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

use crate::protocol::{PacketParseError, PacketType};
use crate::QoS;
// pub use connack::*;
// pub use connect::*;
use crate::protocol::packet::ConnAck;
pub use disconnect::*;
pub use ping::*;
pub use puback::*;
pub use pubcomp::*;
pub use publish::*;
pub use pubrec::*;
pub use pubrel::*;
pub use suback::*;
pub use subscribe::*;
pub use unsuback::*;
pub use unsubscribe::*;

/// Encapsulates all MQTT packet types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubAck),
    PubRec(PubRec),
    PubRel(PubRel),
    PubComp(PubComp),
    SubAck(SubAck),
    UnsubAck(UnsubAck),
    PingResp,
}

impl Packet {
    pub fn packet_ty(&self) -> PacketType {
        match self {
            Packet::ConnAck(_) => PacketType::ConnAck,
            Packet::Publish(_) => PacketType::Publish,
            Packet::PubAck(_) => PacketType::PubAck,
            Packet::PubRec(_) => PacketType::PubRec,
            Packet::PubRel(_) => PacketType::PubRel,
            Packet::PubComp(_) => PacketType::PubComp,
            Packet::SubAck(_) => PacketType::SubAck,
            Packet::UnsubAck(_) => PacketType::UnsubAck,
            Packet::PingResp => PacketType::PingResp,
        }
    }
}

/// Error during serialization and deserialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Expected ConnAck, received: {0:?}")]
    NotConnAck(PacketType),
    #[error("Unexpected Connect")]
    UnexpectedConnect,
    #[error("Invalid Connect return code: {0}")]
    InvalidConnectReturnCode(u8),
    #[error("Invalid protocol")]
    InvalidProtocol,
    #[error("Invalid protocol level: {0}")]
    InvalidProtocolLevel(u8),
    #[error("Incorrect packet format")]
    IncorrectPacketFormat,
    #[error("Invalid packet type: {0}")]
    InvalidPacketType(u8),
    #[error("Invalid property type: {0}")]
    InvalidPropertyType(u8),
    #[error("Invalid QoS level: {0}")]
    InvalidQoS(u8),
    #[error("Invalid subscribe reason code: {0}")]
    InvalidSubscribeReasonCode(u8),
    #[error("Packet id Zero")]
    PacketIdZero,
    #[error("Payload size is incorrect")]
    PayloadSizeIncorrect,
    #[error("payload is too long")]
    PayloadTooLong,
    #[error("payload size limit exceeded: {0}")]
    PayloadSizeLimitExceeded(usize),
    #[error("Payload required")]
    PayloadRequired,
    #[error("Topic is not UTF-8")]
    TopicNotUtf8,
    #[error("Promised boundary crossed: {0}")]
    BoundaryCrossed(usize),
    #[error("Malformed packet")]
    MalformedPacket,
    #[error("A Subscribe packet must contain atleast one filter")]
    EmptySubscription,
    /// More bytes required to frame packet. Argument
    /// implies minimum additional bytes required to
    /// proceed further
    #[error("At least {0} more bytes required to frame packet")]
    InsufficientBytes(usize),
}

/// Error during serialization and deserialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedHeaderError {
    InsufficientBytes(usize),
    MalformedRemainingLength,
}

impl FixedHeaderError {
    pub fn to_discard(&self) -> bool {
        *self == Self::MalformedRemainingLength
    }
}

// pub fn check(stream: Iter<u8>) -> Result<FixedHeader, Error> {
//     let stream_len = stream.len();
//     let fixed_header = parse_fixed_header(stream)?;
//     if fixed_header.remaining_len > max_packet_size {
//         return Err(Error::PayloadSizeLimitExceeded(fixed_header.remaining_len));
//     }
//     let frame_length = fixed_header.frame_length();
//     if stream_len < frame_length {
//         return Err(Error::InsufficientBytes(frame_length - stream_len));
//     }
//     Ok(fixed_header)
// }
