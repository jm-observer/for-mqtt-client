pub(crate) mod connack;
pub(crate) mod connect;
// pub(crate) mod puback;
// pub(crate) mod pubcomp;
mod disconnect;
mod ping;
mod pubcommon;
mod publish;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;

pub use crate::protocol::packet::pubcommon::{PubAck, PubComp, PubRec, PubRel};
pub use crate::protocol::packet::publish::Publish;
pub use crate::protocol::packet::suback::{SubAck, SubscribeReasonCode};
pub(crate) use crate::protocol::packet::subscribe::RetainForwardRule;
pub use crate::protocol::packet::subscribe::Subscribe;
pub use crate::protocol::packet::unsuback::{UnsubAck, UnsubAckReason};
pub use crate::protocol::packet::unsubscribe::Unsubscribe;
use crate::protocol::{FixedHeader, PacketParseError, PacketType, Protocol};
use bytes::{Buf, BufMut, Bytes, BytesMut};
pub use connack::*;
pub use connect::*;
pub use disconnect::*;
pub use ping::*;

use log::error;
use std::slice::Iter;

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

#[derive(Clone, Debug)]
pub enum Packet {
    Connect(Connect),
    ConnAck(ConnAck),
    Publish(Publish),
    PubAck(PubAck),
    // PingReq(PingReq),
    PingResp,
    Subscribe(Subscribe),
    SubAck(SubAck),
    PubRec(PubRec),
    PubRel(PubRel),
    PubComp(PubComp),
    Unsubscribe(Unsubscribe),
    UnsubAck(UnsubAck),
    Disconnect(Disconnect),
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
            Packet::Connect(_) => PacketType::Connect,
            // Packet::PingReq(_) => PacketType::PingReq,
            Packet::Subscribe(_) => PacketType::Subscribe,
            Packet::Unsubscribe(_) => PacketType::Unsubscribe,
            Packet::Disconnect(_) => PacketType::Disconnect,
        }
    }
}

/// 解析包。数据截断、丢弃等逻辑：done
pub fn read_from_network(
    stream: &mut BytesMut,
    version: Protocol,
) -> Result<Packet, PacketParseError> {
    let fixed_header = match parse_fixed_header(stream.iter()) {
        Ok(fixed_header) => {
            if fixed_header.frame_length() > stream.len() {
                error!("fixed_header.frame_length() > stream.len()");
                let _ = stream.split_to(stream.len());
                return Err(FixedHeaderError::MalformedRemainingLength.into());
            }
            fixed_header
        }
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
        PacketType::ConnAck => Packet::ConnAck(ConnAck::read(fixed_header, packet, version)?),
        PacketType::Publish => Packet::Publish(Publish::read(fixed_header, packet, version)?),
        PacketType::PubAck => Packet::PubAck(PubAck::read(fixed_header, packet, version)?),
        PacketType::PubRec => Packet::PubRec(PubRec::read(fixed_header, packet, version)?),
        PacketType::PubRel => Packet::PubRel(PubRel::read(fixed_header, packet, version)?),
        PacketType::PubComp => Packet::PubComp(PubComp::read(fixed_header, packet, version)?),
        PacketType::SubAck => Packet::SubAck(SubAck::read(fixed_header, packet, version)?),
        PacketType::UnsubAck => Packet::UnsubAck(UnsubAck::read(fixed_header, packet, version)?),
        PacketType::PingResp => Packet::PingResp,
        PacketType::Disconnect => match version {
            Protocol::V4 => return Err(PacketParseError::InvalidPacketType(packet_type as u8)),
            Protocol::V5 => Packet::Disconnect(Disconnect::read(fixed_header, packet, version)?),
        },
        ty => {
            error!("{:?}", ty);
            return Err(PacketParseError::InvalidPacketType(packet_type as u8));
        }
    };
    Ok(packet)
}

/// Parses fixed header
pub fn parse_fixed_header(mut stream: Iter<u8>) -> Result<FixedHeader, FixedHeaderError> {
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
pub fn length(stream: Iter<u8>) -> Result<(usize, usize), FixedHeaderError> {
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
pub fn read_mqtt_bytes(stream: &mut Bytes) -> Result<Bytes, PacketParseError> {
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
pub fn read_mqtt_string(stream: &mut Bytes) -> Result<String, PacketParseError> {
    let s = read_mqtt_bytes(stream)?;
    match String::from_utf8(s.to_vec()) {
        Ok(v) => Ok(v),
        Err(_e) => Err(PacketParseError::TopicNotUtf8),
    }
}

/// Serializes bytes to stream (including length)
pub fn write_mqtt_bytes(stream: &mut BytesMut, bytes: &[u8]) {
    stream.put_u16(bytes.len() as u16);
    stream.extend_from_slice(bytes);
}

/// Serializes a string to stream
pub fn write_mqtt_string(stream: &mut BytesMut, string: &str) {
    write_mqtt_bytes(stream, string.as_bytes());
}

/// Writes remaining length to stream and returns number of bytes for remaining length
pub fn write_remaining_length(stream: &mut BytesMut, len: usize) -> usize {
    // only publish packet should be limited and limited at new function now.
    // todo
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
pub fn read_u16(stream: &mut Bytes) -> Result<u16, PacketParseError> {
    if stream.len() < 2 {
        return Err(PacketParseError::MalformedPacket);
    }

    Ok(stream.get_u16())
}

pub fn read_u8(stream: &mut Bytes) -> Result<u8, PacketParseError> {
    if stream.is_empty() {
        return Err(PacketParseError::MalformedPacket);
    }
    Ok(stream.get_u8())
}

pub fn read_u32(stream: &mut Bytes) -> Result<u32, PacketParseError> {
    if stream.len() < 4 {
        return Err(PacketParseError::MalformedPacket);
    }

    Ok(stream.get_u32())
}
