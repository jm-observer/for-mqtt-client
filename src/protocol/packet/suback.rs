use crate::protocol::packet::{length, read_mqtt_string, read_u16, read_u8};
use crate::protocol::{property, FixedHeader, PacketParseError, PropertyType, Protocol};
use crate::QoS;
use bytes::{Buf, Bytes};

/// Acknowledgement to subscribe
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubAck {
    V4 {
        packet_id: u16,
        return_codes: Vec<SubscribeReasonCode>,
    },
    V5ReadMode {
        packet_id: u16,
        properties: Option<SubAckProperties>,
        /// payload
        return_codes: Vec<SubscribeReasonCode>,
    },
}

impl SubAck {
    pub fn packet_id(&self) -> u16 {
        match self {
            SubAck::V4 { packet_id, .. } => *packet_id,
            SubAck::V5ReadMode { packet_id, .. } => *packet_id,
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

                if !bytes.has_remaining() {
                    return Err(PacketParseError::MalformedPacket);
                }

                let mut return_codes = Vec::new();
                while bytes.has_remaining() {
                    let return_code = read_u8(&mut bytes)?;
                    return_codes.push(reason_for_v4(return_code)?);
                }

                SubAck::V4 {
                    packet_id,
                    return_codes,
                }
            }
            Protocol::V5 => {
                let variable_header_index = fixed_header.fixed_header_len;
                bytes.advance(variable_header_index);

                let packet_id = read_u16(&mut bytes)?;
                let properties = SubAckProperties::read(&mut bytes)?;

                if !bytes.has_remaining() {
                    return Err(PacketParseError::MalformedPacket);
                }

                let mut return_codes = Vec::new();
                while bytes.has_remaining() {
                    let return_code = read_u8(&mut bytes)?;
                    return_codes.push(reason_for_v5(return_code)?);
                }

                SubAck::V5ReadMode {
                    packet_id,
                    return_codes,
                    properties,
                }
            }
        };

        Ok(suback)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeReasonCode {
    QoS0,
    QoS1,
    QoS2,
    Success(QoS),
    Failure,
    Unspecified,
    ImplementationSpecific,
    NotAuthorized,
    TopicFilterInvalid,
    PkidInUse,
    QuotaExceeded,
    SharedSubscriptionsNotSupported,
    SubscriptionIdNotSupported,
    WildcardSubscriptionsNotSupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAckProperties {
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
}

impl SubAckProperties {
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

        Ok(Some(SubAckProperties {
            reason_string,
            user_properties,
        }))
    }
}

fn reason_for_v5(code: u8) -> Result<SubscribeReasonCode, PacketParseError> {
    let v = match code {
        0 => SubscribeReasonCode::QoS0,
        1 => SubscribeReasonCode::QoS1,
        2 => SubscribeReasonCode::QoS2,
        128 => SubscribeReasonCode::Unspecified,
        131 => SubscribeReasonCode::ImplementationSpecific,
        135 => SubscribeReasonCode::NotAuthorized,
        143 => SubscribeReasonCode::TopicFilterInvalid,
        145 => SubscribeReasonCode::PkidInUse,
        151 => SubscribeReasonCode::QuotaExceeded,
        158 => SubscribeReasonCode::SharedSubscriptionsNotSupported,
        161 => SubscribeReasonCode::SubscriptionIdNotSupported,
        162 => SubscribeReasonCode::WildcardSubscriptionsNotSupported,
        v => return Err(PacketParseError::InvalidSubscribeReasonCode(v)),
    };
    Ok(v)
}
fn reason_for_v4(code: u8) -> Result<SubscribeReasonCode, PacketParseError> {
    let v = match code {
        0 => SubscribeReasonCode::QoS0,
        1 => SubscribeReasonCode::QoS1,
        2 => SubscribeReasonCode::QoS2,
        128 => SubscribeReasonCode::Unspecified,
        v => return Err(PacketParseError::InvalidSubscribeReasonCode(v)),
    };
    Ok(v)
}
