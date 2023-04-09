use super::*;
use crate::protocol::len_len;
use anyhow::{bail, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::sync::Arc;

impl Default for PubRecReason {
    fn default() -> Self {
        Self::Success
    }
}

pub type PubRel = PubCommon<PubRelTy>;
pub type PubAck = PubCommon<PubAckTy>;
pub type PubRec = PubCommon<PubRecTy>;
pub type PubComp = PubCommon<PubCompTy>;

/// Acknowledgement to QoS1 publish
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PubCommon<Ty: Common> {
    V4 {
        packet_id: u16,
    },
    V5 {
        packet_id: u16,
        reason: Ty::Reason,
        reason_string: Option<String>,
        user_properties: Vec<(String, String)>,
    },
    V5WriteMode {
        packet_id: u16,
        reason: Ty::Reason,
        had_reason_string: bool,
        had_user_properties: bool,
        properties: BytesMut,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubCommonProperties {
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
}

impl<Ty: Common> PubCommon<Ty> {
    pub fn new(packet_id: u16, protocol: Protocol) -> Self {
        match protocol {
            Protocol::V4 => Self::V4 { packet_id },
            Protocol::V5 => Self::V5WriteMode {
                packet_id,
                reason: Ty::Reason::default(),
                had_reason_string: false,
                had_user_properties: false,
                properties: Default::default(),
            },
        }
    }

    pub fn packet_id(&self) -> u16 {
        match self {
            PubCommon::V4 { packet_id, .. } => *packet_id,
            PubCommon::V5 { packet_id, .. } => *packet_id,
            PubCommon::V5WriteMode { packet_id, .. } => *packet_id,
        }
    }

    pub fn set_reason_string(&mut self, reason_string: String) -> Result<()> {
        match self {
            PubCommon::V4 { .. } => {
                bail!("should not be reached")
            }
            PubCommon::V5 { .. } => {
                bail!("should not be reached")
            }
            PubCommon::V5WriteMode {
                had_reason_string,
                properties,
                ..
            } => {
                if *had_reason_string == true {
                    bail!("should not be reached");
                }
                properties.put_u8(PropertyType::ReasonString as u8);
                write_mqtt_string(properties, reason_string.as_str());
                Ok(())
            }
        }
    }

    pub fn add_user_property(&mut self, key: String, val: String) -> Result<()> {
        match self {
            PubCommon::V4 { .. } => {
                bail!("should not be reached")
            }
            PubCommon::V5 { .. } => {
                bail!("should not be reached")
            }
            PubCommon::V5WriteMode {
                had_user_properties,
                properties,
                ..
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
    ) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let packet_id = read_u16(&mut bytes)?;
        match protocol {
            Protocol::V4 => {
                if fixed_header.remaining_len == 2 {
                    return Ok(PubCommon::V4 { packet_id });
                } else {
                    return Err(PacketParseError::MalformedRemainingLength);
                }
            }
            Protocol::V5 => {
                if fixed_header.remaining_len == 2 {
                    return Ok(PubCommon::V5 {
                        packet_id,
                        reason: Ty::Reason::default(),
                        reason_string: None,
                        user_properties: vec![],
                    });
                } else {
                    let ack_reason = read_u8(&mut bytes)?;
                    if fixed_header.remaining_len < 4 {
                        return Ok(PubCommon::V5 {
                            packet_id,
                            reason: Ty::reason(ack_reason)?,
                            reason_string: None,
                            user_properties: vec![],
                        });
                    }
                    let Some(properties) = Self::read_properties(&mut bytes)? else {
                        return Ok(PubCommon::V5 {
                            packet_id,
                            reason: Ty::reason(ack_reason)?,
                            reason_string: None,
                            user_properties: vec![],
                        });
                    };
                    return Ok(PubCommon::V5 {
                        packet_id,
                        reason: Ty::reason(ack_reason)?,
                        reason_string: properties.reason_string,
                        user_properties: properties.user_properties,
                    });
                }
            }
        }
    }

    pub fn read_properties(
        bytes: &mut Bytes,
    ) -> Result<Option<PubCommonProperties>, PacketParseError> {
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

        Ok(Some(PubCommonProperties {
            reason_string,
            user_properties,
        }))
    }
    pub fn data(&self) -> Arc<Bytes> {
        let mut buffer = BytesMut::new();
        self.write(&mut buffer);
        Arc::new(buffer.freeze())
    }
    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        match self {
            PubCommon::V4 { packet_id } => {
                buffer.put_u8(Ty::ty());
                write_remaining_length(buffer, 2);
                buffer.put_u16(*packet_id);
                4
            }
            PubCommon::V5 { .. } => {
                unreachable!()
            }
            PubCommon::V5WriteMode {
                packet_id,
                reason,
                properties,
                ..
            } => {
                let len = self.len();
                buffer.put_u8(Ty::ty());
                let count = write_remaining_length(buffer, len);

                buffer.put_u16(*packet_id);
                if len == 2 {
                    return 4;
                }
                buffer.put_u8(reason.as_u8());
                write_mqtt_bytes(buffer, properties.as_ref());
                len + 1 + count
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            PubCommon::V4 { .. } => 2,
            PubCommon::V5 { .. } => {
                unreachable!()
            }
            PubCommon::V5WriteMode {
                reason,
                had_reason_string,
                had_user_properties,
                properties,
                ..
            } => {
                if reason.is_success() && !*had_reason_string && !*had_user_properties {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PropertyType {
    ReasonString = 31,
    UserProperty = 38,
}

pub trait Common {
    type Reason: Default + Reason;
    fn reason(num: u8) -> Result<Self::Reason, PacketParseError>;
    fn ty() -> u8;
}
pub trait Reason {
    fn is_success(&self) -> bool;
    fn as_u8(&self) -> u8;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PubRecTy;

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

impl Reason for PubRecReason {
    fn is_success(&self) -> bool {
        *self == PubRecReason::Success
    }
    fn as_u8(&self) -> u8 {
        (*self) as u8
    }
}

impl Common for PubRecTy {
    type Reason = PubRecReason;

    fn reason(num: u8) -> Result<Self::Reason, PacketParseError> {
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

    fn ty() -> u8 {
        0x50
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PubRelTy;

/// Return code in connack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PubRelReason {
    Success,
    PacketIdentifierNotFound,
}
impl Reason for PubRelReason {
    fn is_success(&self) -> bool {
        *self == PubRelReason::Success
    }
    fn as_u8(&self) -> u8 {
        (*self) as u8
    }
}

impl Default for PubRelReason {
    fn default() -> Self {
        Self::Success
    }
}

impl Common for PubRelTy {
    type Reason = PubRelReason;

    fn reason(num: u8) -> Result<Self::Reason, PacketParseError> {
        let code = match num {
            0 => PubRelReason::Success,
            146 => PubRelReason::PacketIdentifierNotFound,
            num => return Err(PacketParseError::InvalidConnectReturnCode(num)),
        };
        Ok(code)
    }
    fn ty() -> u8 {
        0x62
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PubCompTy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PubCompReason {
    Success,
    PacketIdentifierNotFound,
}
impl Reason for PubCompReason {
    fn is_success(&self) -> bool {
        *self == PubCompReason::Success
    }
    fn as_u8(&self) -> u8 {
        (*self) as u8
    }
}
impl Default for PubCompReason {
    fn default() -> Self {
        Self::Success
    }
}
impl Common for PubCompTy {
    type Reason = PubCompReason;

    fn reason(num: u8) -> Result<Self::Reason, PacketParseError> {
        let code = match num {
            0 => PubCompReason::Success,
            146 => PubCompReason::PacketIdentifierNotFound,
            num => return Err(PacketParseError::InvalidConnectReturnCode(num)),
        };
        Ok(code)
    }
    fn ty() -> u8 {
        0x70
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PubAckTy;

impl Reason for PubAckReason {
    fn is_success(&self) -> bool {
        *self == PubAckReason::Success
    }
    fn as_u8(&self) -> u8 {
        (*self) as u8
    }
}
impl Default for PubAckReason {
    fn default() -> Self {
        Self::Success
    }
}
impl Common for PubAckTy {
    type Reason = PubAckReason;

    fn reason(num: u8) -> Result<Self::Reason, PacketParseError> {
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

    fn ty() -> u8 {
        0x40
    }
}
