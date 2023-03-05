use crate::protocol::packet::{length, read_mqtt_string, read_u16, read_u8};
use crate::protocol::{property, FixedHeader, PacketParseError, PropertyType, Protocol};

use bytes::{Buf, Bytes};

/// Acknowledgement to subscribe
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsubAck {
    V4 {
        packet_id: u16,
    },
    V5ReadMode {
        packet_id: u16,
        properties: Option<UnsubAckProperties>,
        /// payload
        return_codes: Vec<UnsubAckReason>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UnsubAckReason {
    Success,
    NoSubscriptionExisted,
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicFilterInvalid,
    PacketIdentifierInUse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubAckProperties {
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
}

impl UnsubAck {
    pub fn packet_id(&self) -> u16 {
        match self {
            UnsubAck::V4 { packet_id, .. } => *packet_id,
            UnsubAck::V5ReadMode { packet_id, .. } => *packet_id,
        }
    }
    pub fn read(
        fixed_header: FixedHeader,
        mut bytes: Bytes,
        protocal: Protocol,
    ) -> Result<Self, PacketParseError> {
        let suback = match protocal {
            Protocol::V4 => {
                let variable_header_index = fixed_header.fixed_header_len;
                bytes.advance(variable_header_index);
                let packet_id = read_u16(&mut bytes)?;
                UnsubAck::V4 { packet_id }
            }
            Protocol::V5 => {
                let variable_header_index = fixed_header.fixed_header_len;
                bytes.advance(variable_header_index);

                let packet_id = read_u16(&mut bytes)?;
                let properties = UnsubAckProperties::read(&mut bytes)?;

                if !bytes.has_remaining() {
                    return Err(PacketParseError::MalformedPacket);
                }

                let mut return_codes = Vec::new();
                while bytes.has_remaining() {
                    let return_code = read_u8(&mut bytes)?;
                    return_codes.push(reason_for_v5(return_code)?);
                }

                UnsubAck::V5ReadMode {
                    packet_id,
                    return_codes,
                    properties,
                }
            }
        };

        Ok(suback)
    }
}

impl UnsubAckProperties {
    pub fn read(bytes: &mut Bytes) -> Result<Option<Self>, PacketParseError> {
        let mut reason_string = None;
        let mut user_properties = Vec::new();

        let (properties_len_len, properties_len) = length(bytes.iter())?;
        bytes.advance(properties_len_len);
        if properties_len == 0 {
            return Ok(None);
        }

        let mut cursor = 0;
        // read until cursor reaches property length. properties_len = 0 will skip this loop
        while cursor < properties_len {
            let prop = read_u8(bytes)?;
            cursor += 1;

            match property(prop)? {
                PropertyType::ReasonString => {
                    let reason = read_mqtt_string(bytes)?;
                    cursor += 2 + reason.len();
                    reason_string = Some(reason);
                }
                PropertyType::UserProperty => {
                    let key = read_mqtt_string(bytes)?;
                    let value = read_mqtt_string(bytes)?;
                    cursor += 2 + key.len() + 2 + value.len();
                    user_properties.push((key, value));
                }
                _ => return Err(PacketParseError::InvalidPropertyType(prop)),
            }
        }

        Ok(Some(UnsubAckProperties {
            reason_string,
            user_properties,
        }))
    }
}

/// Connection return code type
fn reason_for_v5(num: u8) -> Result<UnsubAckReason, PacketParseError> {
    let code = match num {
        0x00 => UnsubAckReason::Success,
        0x11 => UnsubAckReason::NoSubscriptionExisted,
        0x80 => UnsubAckReason::UnspecifiedError,
        0x83 => UnsubAckReason::ImplementationSpecificError,
        0x87 => UnsubAckReason::NotAuthorized,
        0x8F => UnsubAckReason::TopicFilterInvalid,
        0x91 => UnsubAckReason::PacketIdentifierInUse,
        num => return Err(PacketParseError::InvalidSubscribeReasonCode(num)),
    };

    Ok(code)
}
