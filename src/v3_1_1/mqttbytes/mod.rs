//! # mqttbytes
//!
//! This module contains the low level struct definitions required to assemble and disassemble MQTT 3.1.1 packets in rumqttc.
//! The [`bytes`](https://docs.rs/bytes) crate is used internally.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use core::fmt;
use std::slice::Iter;

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

use crate::protocol::PacketParseError;
use crate::QoS;
pub use connack::*;
pub use connect::*;
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

/// 解析包。数据截断、丢弃等逻辑：done
pub fn read_from_network(stream: &mut BytesMut) -> Result<Packet, PacketParseError> {
    let fixed_header = match parse_fixed_header(stream.iter()) {
        Ok(fixed_header) => fixed_header,
        Err(err) => {
            if err.to_discard() {
                // discard
                let _ = stream.split_to(stream.len());
            }
            return Err(err.into());
        }
    };
    let packet = stream.split_to(fixed_header.frame_length());
    let packet_type = fixed_header.packet_type()?;
    let packet = packet.freeze();
    let packet = match packet_type {
        PacketType::ConnAck => Packet::ConnAck(ConnAck::read(fixed_header, packet)?),
        PacketType::Publish => Packet::Publish(Publish::read(fixed_header, packet)?),
        PacketType::PubAck => Packet::PubAck(PubAck::read(fixed_header, packet)?),
        PacketType::PubRec => Packet::PubRec(PubRec::read(fixed_header, packet)?),
        PacketType::PubRel => Packet::PubRel(PubRel::read(fixed_header, packet)?),
        PacketType::PubComp => Packet::PubComp(PubComp::read(fixed_header, packet)?),
        PacketType::SubAck => Packet::SubAck(SubAck::read(fixed_header, packet)?),
        PacketType::UnsubAck => Packet::UnsubAck(UnsubAck::read(fixed_header, packet)?),
        PacketType::PingResp => Packet::PingResp,
        _ => return Err(PacketParseError::InvalidPacketType(packet_type as u8)),
    };
    Ok(packet)
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

/// Parses fixed header
fn parse_fixed_header(mut stream: Iter<u8>) -> Result<FixedHeader, FixedHeaderError> {
    // At least 2 bytes are necessary to frame a packet
    let stream_len = stream.len();
    if stream_len < 2 {
        return Err(FixedHeaderError::InsufficientBytes(2 - stream_len));
    }
    let byte1 = stream
        .next()
        .ok_or(FixedHeaderError::InsufficientBytes(2))?;
    let (len_len, len) = length(stream)?;

    Ok(FixedHeader::new(*byte1, len_len, len))
}

/// Parses variable byte integer in the stream and returns the length
/// and number of bytes that make it. Used for remaining length calculation
/// as well as for calculating property lengths
fn length(stream: Iter<u8>) -> Result<(usize, usize), FixedHeaderError> {
    let mut len: usize = 0;
    let mut len_len = 0;
    let mut done = false;
    let mut shift = 0;

    // Use continuation bit at position 7 to continue reading next
    // byte to frame 'length'.
    // Stream 0b1xxx_xxxx 0b1yyy_yyyy 0b1zzz_zzzz 0b0www_wwww will
    // be framed as number 0bwww_wwww_zzz_zzzz_yyy_yyyy_xxx_xxxx
    for byte in stream {
        len_len += 1;
        let byte = *byte as usize;
        len += (byte & 0x7F) << shift;

        // stop when continue bit is 0
        done = (byte & 0x80) == 0;
        if done {
            break;
        }

        shift += 7;

        // Only a max of 4 bytes allowed for remaining length
        // more than 4 shifts (0, 7, 14, 21) implies bad length
        if shift > 21 {
            return Err(FixedHeaderError::MalformedRemainingLength);
        }
    }

    // Not enough bytes to frame remaining length. wait for
    // one more byte
    if !done {
        return Err(FixedHeaderError::InsufficientBytes(1));
    }

    Ok((len_len, len))
}

/// Reads a series of bytes with a length from a byte stream
fn read_mqtt_bytes(stream: &mut Bytes) -> Result<Bytes, PacketParseError> {
    let len = read_u16(stream)? as usize;

    // Prevent attacks with wrong remaining length. This method is used in
    // `packet.assembly()` with (enough) bytes to frame packet. Ensures that
    // reading variable len string or bytes doesn't cross promised boundary
    // with `read_fixed_header()`
    if len > stream.len() {
        return Err(PacketParseError::BoundaryCrossed(len));
    }

    Ok(stream.split_to(len))
}

/// Reads a string from bytes stream
fn read_mqtt_string(stream: &mut Bytes) -> Result<String, PacketParseError> {
    let s = read_mqtt_bytes(stream)?;
    match String::from_utf8(s.to_vec()) {
        Ok(v) => Ok(v),
        Err(_e) => Err(PacketParseError::TopicNotUtf8),
    }
}

/// Serializes bytes to stream (including length)
fn write_mqtt_bytes(stream: &mut BytesMut, bytes: &[u8]) {
    stream.put_u16(bytes.len() as u16);
    stream.extend_from_slice(bytes);
}

/// Serializes a string to stream
fn write_mqtt_string(stream: &mut BytesMut, string: &str) {
    write_mqtt_bytes(stream, string.as_bytes());
}

/// Writes remaining length to stream and returns number of bytes for remaining length
fn write_remaining_length(stream: &mut BytesMut, len: usize) -> usize {
    // only publish packet should be limited and limited at new function now.
    // if len > 268_435_455 {
    //     return Err(Error::PayloadTooLong);
    // }

    let mut done = false;
    let mut x = len;
    let mut count = 0;

    while !done {
        let mut byte = (x % 128) as u8;
        x /= 128;
        if x > 0 {
            byte |= 128;
        }

        stream.put_u8(byte);
        count += 1;
        done = x == 0;
    }
    count
}

/// After collecting enough bytes to frame a packet (packet's frame())
/// , It's possible that content itself in the stream is wrong. Like expected
/// packet id or qos not being present. In cases where `read_mqtt_string` or
/// `read_mqtt_bytes` exhausted remaining length but packet framing expects to
/// parse qos next, these pre checks will prevent `bytes` crashes
fn read_u16(stream: &mut Bytes) -> Result<u16, PacketParseError> {
    if stream.len() < 2 {
        return Err(PacketParseError::MalformedPacket);
    }

    Ok(stream.get_u16())
}

fn read_u8(stream: &mut Bytes) -> Result<u8, PacketParseError> {
    if stream.is_empty() {
        return Err(PacketParseError::MalformedPacket);
    }
    Ok(stream.get_u8())
}
