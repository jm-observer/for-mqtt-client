use super::*;
use crate::protocol::packet::{read_u16, write_remaining_length};
use crate::protocol::{len_len, property, FixedHeader, PropertyType};
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::sync::Arc;

/// Acknowledgement to QoS1 publish
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubAck {
    pub protocol: Protocol,
    pub packet_id: u16,
    pub reason: Option<PubAckReason>,
    pub properties: Option<PubAckProperties>,
}

impl PubAck {
    pub fn data(packet_id: u16, protocol: Protocol) -> Arc<Bytes> {
        let packet = match &protocol {
            Protocol::V4 => PubAck {
                packet_id,
                protocol,
                reason: None,
                properties: None,
            },
            Protocol::V5 => PubAck {
                packet_id,
                protocol,
                reason: Some(PubAckReason::Success),
                properties: None,
            },
        };
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes);
        bytes.freeze().into()
    }

    fn len(&self) -> usize {
        match self.protocol {
            Protocol::V4 => 2,
            Protocol::V5 => {
                let mut len = 2 + 1; // pkid + reason

                // If there are no properties, sending reason code is optional
                if let Some(reason) = &self.reason {
                    if self.properties.is_none() && *reason == PubAckReason::Success {
                        return 2;
                    }
                }

                if let Some(p) = &self.properties {
                    let properties_len = p.len();
                    let properties_len_len = len_len(properties_len);
                    len += properties_len_len + properties_len;
                } else {
                    // just 1 byte representing 0 len properties
                    len += 1;
                }
                len
            }
        }
    }

    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        let len = self.len();
        buffer.put_u8(0x40);
        let count = write_remaining_length(buffer, len);
        buffer.put_u16(self.packet_id);

        match &self.protocol {
            Protocol::V4 => 1 + count + len,
            Protocol::V5 => {
                if let Some(reason) = &self.reason {
                    if self.properties.is_none() && *reason == PubAckReason::Success {
                        return 4;
                    }
                    buffer.put_u8(code(*reason));
                }
                if let Some(p) = &self.properties {
                    p.write(buffer);
                } else {
                    write_remaining_length(buffer, 0);
                }
                1 + count + len
            }
        }
    }

    pub fn read(
        fixed_header: FixedHeader,
        mut bytes: Bytes,
        protocol: Protocol,
    ) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let packet_id = read_u16(&mut bytes)?;

        let puback = match &protocol {
            Protocol::V4 => Self {
                protocol,
                packet_id,
                reason: None,
                properties: None,
            },
            Protocol::V5 => {
                // No reason code or properties if remaining length == 2
                let reason = if fixed_header.remaining_len == 2 {
                    PubAckReason::Success
                } else {
                    let ack_reason = read_u8(&mut bytes)?;
                    reason(ack_reason)?
                };
                let properties = if fixed_header.remaining_len >= 4 {
                    None
                } else {
                    PubAckProperties::read(&mut bytes)?
                };
                Self {
                    protocol,
                    packet_id,
                    reason: Some(reason),
                    properties,
                }
            }
        };
        Ok(puback)
    }
}

/// Return code in puback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PubAckReason {
    Success,
    NoMatchingSubscribers,
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicNameInvalid,
    PacketIdentifierInUse,
    QuotaExceeded,
    PayloadFormatInvalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubAckProperties {
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
}

impl PubAckProperties {
    pub fn len(&self) -> usize {
        let mut len = 0;

        if let Some(reason) = &self.reason_string {
            len += 1 + 2 + reason.len();
        }

        for (key, value) in self.user_properties.iter() {
            len += 1 + 2 + key.len() + 2 + value.len();
        }

        len
    }

    pub fn write(&self, buffer: &mut BytesMut) {
        let len = self.len();
        write_remaining_length(buffer, len);

        if let Some(reason) = &self.reason_string {
            buffer.put_u8(PropertyType::ReasonString as u8);
            write_mqtt_string(buffer, reason);
        }

        for (key, value) in self.user_properties.iter() {
            buffer.put_u8(PropertyType::UserProperty as u8);
            write_mqtt_string(buffer, key);
            write_mqtt_string(buffer, value);
        }
    }

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

        Ok(Some(PubAckProperties {
            reason_string,
            user_properties,
        }))
    }
}

/// Connection return code type
fn reason(num: u8) -> Result<PubAckReason, PacketParseError> {
    let code = match num {
        0 => PubAckReason::Success,
        16 => PubAckReason::NoMatchingSubscribers,
        128 => PubAckReason::UnspecifiedError,
        131 => PubAckReason::ImplementationSpecificError,
        135 => PubAckReason::NotAuthorized,
        144 => PubAckReason::TopicNameInvalid,
        145 => PubAckReason::PacketIdentifierInUse,
        151 => PubAckReason::QuotaExceeded,
        153 => PubAckReason::PayloadFormatInvalid,
        num => return Err(PacketParseError::InvalidConnectReturnCode(num)),
    };

    Ok(code)
}

fn code(reason: PubAckReason) -> u8 {
    match reason {
        PubAckReason::Success => 0,
        PubAckReason::NoMatchingSubscribers => 16,
        PubAckReason::UnspecifiedError => 128,
        PubAckReason::ImplementationSpecificError => 131,
        PubAckReason::NotAuthorized => 135,
        PubAckReason::TopicNameInvalid => 144,
        PubAckReason::PacketIdentifierInUse => 145,
        PubAckReason::QuotaExceeded => 151,
        PubAckReason::PayloadFormatInvalid => 153,
    }
}
