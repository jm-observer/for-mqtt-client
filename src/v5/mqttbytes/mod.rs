use std::slice::Iter;

use self::{disconnect::Disconnect, ping::pingreq};

use super::*;
use bytes::{Buf, BufMut, Bytes, BytesMut};

mod connack;
mod connect;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Packet {
    Connect(
        Connect,
        Option<ConnectProperties>,
        Option<LastWill>,
        Option<LastWillProperties>,
        Option<Login>,
    ),
    ConnAck(ConnAck),
    Publish(Publish, Option<PublishProperties>),
    PubAck(PubAck, Option<PubAckProperties>),
    PingReq(PingReq),
    PingResp(PingResp),
    Subscribe(Subscribe, Option<SubscribeProperties>),
    SubAck(SubAck, Option<SubAckProperties>),
    PubRec(PubRec, Option<PubRecProperties>),
    PubRel(PubRel, Option<PubRelProperties>),
    PubComp(PubComp, Option<PubCompProperties>),
    Unsubscribe(Unsubscribe),
    UnsubAck(UnsubAck),
    Disconnect(Disconnect),
}

impl Packet {
    /// Reads a stream of bytes and extracts next MQTT packet out of it
    pub fn read(stream: &mut BytesMut, max_size: usize) -> Result<Packet, Error> {
        let fixed_header = check(stream.iter(), max_size)?;

        // Test with a stream with exactly the size to check border panics
        let packet = stream.split_to(fixed_header.frame_length());
        let packet_type = fixed_header.packet_type()?;

        if fixed_header.remaining_len == 0 {
            // no payload packets, Disconnect still has a bit more info
            return match packet_type {
                PacketType::PingReq => Ok(Packet::PingReq(PingReq)),
                PacketType::PingResp => Ok(Packet::PingResp(PingResp)),
                _ => Err(Error::PayloadRequired),
            };
        }

        let packet = packet.freeze();
        let packet = match packet_type {
            PacketType::Connect => {
                let (connect, properties, will, willproperties, login) =
                    connect::read(fixed_header, packet)?;
                Packet::Connect(connect, properties, will, willproperties, login)
            }
            PacketType::Publish => {
                let (publish, properties) = publish::read(fixed_header, packet)?;
                Packet::Publish(publish, properties)
            }
            PacketType::Subscribe => {
                let (subscribe, properties) = subscribe::read(fixed_header, packet)?;
                Packet::Subscribe(subscribe, properties)
            }
            PacketType::Unsubscribe => {
                let (unsubscribe, _) = unsubscribe::read(fixed_header, packet)?;
                Packet::Unsubscribe(unsubscribe)
            }
            PacketType::ConnAck => {
                let (connack, _) = connack::read(fixed_header, packet)?;
                Packet::ConnAck(connack)
            }
            PacketType::PubAck => {
                let (puback, properties) = puback::read(fixed_header, packet)?;
                Packet::PubAck(puback, properties)
            }
            PacketType::PubRec => {
                let (pubrec, properties) = pubrec::read(fixed_header, packet)?;
                Packet::PubRec(pubrec, properties)
            }
            PacketType::PubRel => {
                let (pubrel, properties) = pubrel::read(fixed_header, packet)?;
                Packet::PubRel(pubrel, properties)
            }
            PacketType::PubComp => {
                let (pubcomp, properties) = pubcomp::read(fixed_header, packet)?;
                Packet::PubComp(pubcomp, properties)
            }
            PacketType::SubAck => {
                let (suback, properties) = suback::read(fixed_header, packet)?;
                Packet::SubAck(suback, properties)
            }
            PacketType::UnsubAck => {
                let (unsuback, _) = unsuback::read(fixed_header, packet)?;
                Packet::UnsubAck(unsuback)
            }
            PacketType::PingReq => Packet::PingReq(PingReq),
            PacketType::PingResp => Packet::PingResp(PingResp),
            PacketType::Disconnect => {
                let disconnect = Disconnect::read(fixed_header, packet)?;
                Packet::Disconnect(disconnect)
            }
        };

        Ok(packet)
    }

    pub fn write(&self, write: &mut BytesMut) -> Result<usize, Error> {
        match self {
            Self::Publish(publish, properties) => publish::write(publish, properties, write),
            Self::Subscribe(subscription, properties) => {
                subscribe::write(subscription, properties, write)
            }
            Self::Unsubscribe(unsubscribe) => unsubscribe::write(unsubscribe, &None, write),
            Self::ConnAck(ack) => connack::write(ack, &None, write),
            Self::PubAck(ack, properties) => puback::write(ack, properties, write),
            Self::SubAck(ack, properties) => suback::write(ack, properties, write),
            Self::UnsubAck(unsuback) => unsuback::write(unsuback, &None, write),
            Self::PubRec(pubrec, properties) => pubrec::write(pubrec, properties, write),
            Self::PubRel(pubrel, properties) => pubrel::write(pubrel, properties, write),
            Self::PubComp(pubcomp, properties) => pubcomp::write(pubcomp, properties, write),
            Self::Connect(connect, properties, will, will_properties, login) => {
                connect::write(connect, will, will_properties, login, properties, write)
            }
            Self::PingReq(_) => pingreq::write(write),
            Self::PingResp(_) => ping::pingresp::write(write),
            Self::Disconnect(disconnect) => disconnect.write(write),
        }
    }
}

/// MQTT packet type
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Connect = 1,
    ConnAck,
    Publish,
    PubAck,
    PubRec,
    PubRel,
    PubComp,
    Subscribe,
    SubAck,
    Unsubscribe,
    UnsubAck,
    PingReq,
    PingResp,
    Disconnect,
}

/// Packet type from a byte
///
/// ```ignore
///          7                          3                          0
///          +--------------------------+--------------------------+
/// byte 1   | MQTT Control Packet Type | Flags for each type      |
///          +--------------------------+--------------------------+
///          |         Remaining Bytes Len  (1/2/3/4 bytes)        |
///          +-----------------------------------------------------+
///
/// <https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html#_Toc385349207>
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct FixedHeader {
    /// First byte of the stream. Used to identify packet types and
    /// several flags
    byte1: u8,
    /// Length of fixed header. Byte 1 + (1..4) bytes. So fixed header
    /// len can vary from 2 bytes to 5 bytes
    /// 1..4 bytes are variable length encoded to represent remaining length
    fixed_header_len: usize,
    /// Remaining length of the packet. Doesn't include fixed header bytes
    /// Represents variable header + payload size
    remaining_len: usize,
}

impl FixedHeader {
    pub fn new(byte1: u8, remaining_len_len: usize, remaining_len: usize) -> FixedHeader {
        FixedHeader {
            byte1,
            fixed_header_len: remaining_len_len + 1,
            remaining_len,
        }
    }

    pub fn packet_type(&self) -> Result<PacketType, Error> {
        let num = self.byte1 >> 4;
        match num {
            1 => Ok(PacketType::Connect),
            2 => Ok(PacketType::ConnAck),
            3 => Ok(PacketType::Publish),
            4 => Ok(PacketType::PubAck),
            5 => Ok(PacketType::PubRec),
            6 => Ok(PacketType::PubRel),
            7 => Ok(PacketType::PubComp),
            8 => Ok(PacketType::Subscribe),
            9 => Ok(PacketType::SubAck),
            10 => Ok(PacketType::Unsubscribe),
            11 => Ok(PacketType::UnsubAck),
            12 => Ok(PacketType::PingReq),
            13 => Ok(PacketType::PingResp),
            14 => Ok(PacketType::Disconnect),
            _ => Err(Error::InvalidPacketType(num)),
        }
    }

    /// Returns the size of full packet (fixed header + variable header + payload)
    /// Fixed header is enough to get the size of a frame in the stream
    pub fn frame_length(&self) -> usize {
        self.fixed_header_len + self.remaining_len
    }
}
