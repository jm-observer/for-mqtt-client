use super::*;
use crate::protocol::len_len;
use anyhow::{bail, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Return code in connack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PubRecReason {
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

/// Acknowledgement to QoS1 publish
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubRec {
    V4 {
        packet_id: u16,
    },
    V5 {
        packet_id: u16,
        reason: PubRecReason,
        reason_string: Option<String>,
        user_properties: Vec<(String, String)>,
    },
    V5WriteMode {
        packet_id: u16,
        reason: PubRecReason,
        had_reason_string: bool,
        had_user_properties: bool,
        properties: BytesMut,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubRecProperties {
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
}

impl PubRec {
    pub fn new(packet_id: u16, protocol: Protocol) -> Self {
        match protocol {
            Protocol::V4 => Self::V4 { packet_id },
            Protocol::V5 => Self::V5WriteMode {
                packet_id,
                reason: PubRecReason::Success,
                had_reason_string: false,
                had_user_properties: false,
                properties: Default::default(),
            },
        }
    }

    pub fn set_reason_string(&mut self, reason_string: String) -> Result<()> {
        match self {
            PubRec::V4 { .. } => {
                bail!("todo")
            }
            PubRec::V5 { .. } => {
                bail!("todo")
            }
            PubRec::V5WriteMode {
                packet_id,
                reason,
                had_reason_string,
                had_user_properties,
                properties,
            } => {
                if *had_reason_string == true {
                    bail!("todo");
                }
                properties.put_u8(PropertyType::ReasonString as u8);
                write_mqtt_string(properties, reason_string.as_str());
                Ok(())
            }
        }
    }

    pub fn add_user_property(&mut self, key: String, val: String) -> Result<()> {
        match self {
            PubRec::V4 { .. } => {
                bail!("todo")
            }
            PubRec::V5 { .. } => {
                bail!("todo")
            }
            PubRec::V5WriteMode {
                packet_id,
                reason,
                had_reason_string,
                had_user_properties,
                properties,
            } => {
                *had_user_properties = true;
                properties.put_u8(PropertyType::UserProperty as u8);
                write_mqtt_string(properties, key.as_str());
                write_mqtt_string(properties, val.as_str());
                Ok(())
            }
        }
    }

    pub fn read(
        fixed_header: FixedHeader,
        mut bytes: Bytes,
        protocol: Protocol,
    ) -> Result<PubRec, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let packet_id = read_u16(&mut bytes)?;
        match protocol {
            Protocol::V4 => {
                if fixed_header.remaining_len == 2 {
                    return Ok(PubRec::V4 { packet_id });
                } else {
                    return Err(PacketParseError::MalformedRemainingLength);
                }
            }
            Protocol::V5 => {
                if fixed_header.remaining_len == 2 {
                    return Ok(PubRec::V5 {
                        packet_id,
                        reason: PubRecReason::Success,
                        reason_string: None,
                        user_properties: vec![],
                    });
                } else {
                    let ack_reason = read_u8(&mut bytes)?;
                    if fixed_header.remaining_len < 4 {
                        return Ok(PubRec::V5 {
                            packet_id,
                            reason: reason(ack_reason)?,
                            reason_string: None,
                            user_properties: vec![],
                        });
                    }
                    let Some(properties) = Self::read_properties(&mut bytes)? else {
                        return Ok(PubRec::V5 {
                            packet_id,
                            reason: reason(ack_reason)?,
                            reason_string: None,
                            user_properties: vec![],
                        });
                    };
                    return Ok(PubRec::V5 {
                        packet_id,
                        reason: reason(ack_reason)?,
                        reason_string: properties.reason_string,
                        user_properties: properties.user_properties,
                    });
                }
            }
        }
    }

    pub fn read_properties(
        bytes: &mut Bytes,
    ) -> Result<Option<PubRecProperties>, PacketParseError> {
        let mut reason_string = None;
        let mut user_properties = Vec::new();
        let (properties_len_len, properties_len) = length(bytes.iter())?;
        bytes.advance(properties_len_len);
        if properties_len == 0 {
            return Ok(None);
        }
        let mut cursor = 0;
        while cursor < properties_len {
            let prop = read_u8(bytes)?;
            cursor += 1;

            match prop {
                31 => {
                    let reason = read_mqtt_string(bytes)?;
                    cursor += 2 + reason.len();
                    reason_string = Some(reason);
                }
                38 => {
                    let key = read_mqtt_string(bytes)?;
                    let value = read_mqtt_string(bytes)?;
                    cursor += 2 + key.len() + 2 + value.len();
                    user_properties.push((key, value));
                }
                _ => return Err(PacketParseError::InvalidPropertyType(prop)),
            }
        }

        Ok(Some(PubRecProperties {
            reason_string,
            user_properties,
        }))
    }

    pub fn write(&self, buffer: &mut BytesMut) -> Result<usize, PacketParseError> {
        match self {
            PubRec::V4 { packet_id } => {
                buffer.put_u8(0x50);
                let count = write_remaining_length(buffer, 2);
                buffer.put_u16(*packet_id);
                Ok(4)
            }
            PubRec::V5 { .. } => {
                unreachable!()
            }
            PubRec::V5WriteMode {
                packet_id,
                reason,
                had_reason_string,
                had_user_properties,
                properties,
            } => {
                let len = self.len();
                buffer.put_u8(0x50);
                let count = write_remaining_length(buffer, len);

                buffer.put_u16(*packet_id);
                if len == 2 {
                    return Ok(4);
                }
                if len == 2 {
                    return Ok(4);
                }
                buffer.put_u8((*reason) as u8);
                write_mqtt_bytes(buffer, properties.as_ref());
                Ok(len + 1 + count)
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            PubRec::V4 { .. } => 2,
            PubRec::V5 { .. } => {
                unreachable!()
            }
            PubRec::V5WriteMode {
                reason,
                had_reason_string,
                had_user_properties,
                properties,
                ..
            } => {
                if *reason == PubRecReason::Success && !*had_reason_string && !*had_user_properties
                {
                    return 2;
                }
                let mut len = 2 + 1; // packet_id + reason
                if *had_reason_string && *had_user_properties {
                    let properties_len = properties.len();
                    let properties_len_len = len_len(properties_len);
                    len += properties_len_len + properties_len;
                }
                len
            }
        }
        //
        // if let Some(p) = properties {
        //     let properties_len = properties::len(p);
        //     let properties_len_len = len_len(properties_len);
        //     len += properties_len_len + properties_len;
        // } else {
        //     len += 1
        // }
    }
}
/// Connection return code type
fn reason(num: u8) -> Result<PubRecReason, PacketParseError> {
    let code = match num {
        0 => PubRecReason::Success,
        16 => PubRecReason::NoMatchingSubscribers,
        128 => PubRecReason::UnspecifiedError,
        131 => PubRecReason::ImplementationSpecificError,
        135 => PubRecReason::NotAuthorized,
        144 => PubRecReason::TopicNameInvalid,
        145 => PubRecReason::PacketIdentifierInUse,
        151 => PubRecReason::QuotaExceeded,
        153 => PubRecReason::PayloadFormatInvalid,
        num => return Err(PacketParseError::InvalidConnectReturnCode(num)),
    };

    Ok(code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PropertyType {
    ReasonString = 31,
    UserProperty = 38,
}
