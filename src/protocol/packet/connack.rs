use crate::protocol::packet::read_u8;
use crate::protocol::{FixedHeader, PacketParseError, Protocol};
use bytes::{Buf, Bytes};

/// Acknowledgement to connect packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnAck {
    pub session_present: bool,
    pub code: ConnectReturnCode,
}

impl ConnAck {
    pub fn read(
        fixed_header: FixedHeader,
        mut bytes: Bytes,
        version: &Protocol,
    ) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);

        let flags = read_u8(&mut bytes)?;
        let return_code = read_u8(&mut bytes)?;

        let session_present = (flags & 0x01) == 1;
        let code = match version {
            Protocol::V4 => connect_return_v3(return_code)?,
            Protocol::V5 => connect_return_v5(return_code)?,
        };
        let conn_ack = ConnAck {
            session_present,
            code,
        };
        Ok(conn_ack)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnAckProperties {
    pub session_expiry_interval: Option<u32>,
    pub receive_max: Option<u16>,
    pub max_qos: Option<u8>,
    pub retain_available: Option<u8>,
    pub max_packet_size: Option<u32>,
    pub assigned_client_identifier: Option<String>,
    pub topic_alias_max: Option<u16>,
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
    pub wildcard_subscription_available: Option<u8>,
    pub subscription_identifiers_available: Option<u8>,
    pub shared_subscription_available: Option<u8>,
    pub server_keep_alive: Option<u16>,
    pub response_information: Option<String>,
    pub server_reference: Option<String>,
    pub authentication_method: Option<String>,
    pub authentication_data: Option<Bytes>,
}

/// Return code in conn ack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReturnCode {
    Success,
    Fail(ConnectReturnFailCode),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReturnFailCode {
    FailV3(ConnectReturnFailCodeV3),
    FailV5(ConnectReturnCodeV5),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConnectReturnFailCodeV3 {
    RefusedProtocolVersion = 1,
    /// The Client identifier is correct UTF-8 but not allowed by the Server
    BadClientId,
    ServiceUnavailable,
    BadUserNamePassword,
    NotAuthorized,
}

/// Connection return code type
fn connect_return_v3(num: u8) -> Result<ConnectReturnCode, PacketParseError> {
    match num {
        0 => Ok(ConnectReturnCode::Success),
        1 => Ok(ConnectReturnFailCodeV3::RefusedProtocolVersion.into()),
        2 => Ok(ConnectReturnFailCodeV3::BadClientId.into()),
        3 => Ok(ConnectReturnFailCodeV3::ServiceUnavailable.into()),
        4 => Ok(ConnectReturnFailCodeV3::BadUserNamePassword.into()),
        5 => Ok(ConnectReturnFailCodeV3::NotAuthorized.into()),
        num => Err(PacketParseError::InvalidConnectReturnCode(num)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectReturnCodeV5 {
    RefusedProtocolVersion,
    BadClientId,
    ServiceUnavailable,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    UnsupportedProtocolVersion,
    ClientIdentifierNotValid,
    BadUserNamePassword,
    NotAuthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    BadAuthenticationMethod,
    TopicNameInvalid,
    PacketTooLarge,
    QuotaExceeded,
    PayloadFormatInvalid,
    RetainNotSupported,
    QoSNotSupported,
    UseAnotherServer,
    ServerMoved,
    ConnectionRateExceeded,
}

/// Connection return code type
fn connect_return_v5(num: u8) -> Result<ConnectReturnCode, PacketParseError> {
    let code = match num {
        0 => ConnectReturnCode::Success,
        128 => ConnectReturnCodeV5::UnspecifiedError.into(),
        129 => ConnectReturnCodeV5::MalformedPacket.into(),
        130 => ConnectReturnCodeV5::ProtocolError.into(),
        131 => ConnectReturnCodeV5::ImplementationSpecificError.into(),
        132 => ConnectReturnCodeV5::UnsupportedProtocolVersion.into(),
        133 => ConnectReturnCodeV5::ClientIdentifierNotValid.into(),
        134 => ConnectReturnCodeV5::BadUserNamePassword.into(),
        135 => ConnectReturnCodeV5::NotAuthorized.into(),
        136 => ConnectReturnCodeV5::ServerUnavailable.into(),
        137 => ConnectReturnCodeV5::ServerBusy.into(),
        138 => ConnectReturnCodeV5::Banned.into(),
        140 => ConnectReturnCodeV5::BadAuthenticationMethod.into(),
        144 => ConnectReturnCodeV5::TopicNameInvalid.into(),
        149 => ConnectReturnCodeV5::PacketTooLarge.into(),
        151 => ConnectReturnCodeV5::QuotaExceeded.into(),
        153 => ConnectReturnCodeV5::PayloadFormatInvalid.into(),
        154 => ConnectReturnCodeV5::RetainNotSupported.into(),
        155 => ConnectReturnCodeV5::QoSNotSupported.into(),
        156 => ConnectReturnCodeV5::UseAnotherServer.into(),
        157 => ConnectReturnCodeV5::ServerMoved.into(),
        159 => ConnectReturnCodeV5::ConnectionRateExceeded.into(),
        num => return Err(PacketParseError::InvalidConnectReturnCode(num)),
    };

    Ok(code)
}

//
// pub enum ConnectReturnCode {
//     Success = 0,
//     Fail(ConnectReturnFailCode),
// }
// pub enum ConnectReturnFailCode {
//     FailV3(ConnectReturnFailCodeV3),
//     FailV5(ConnectReturnCodeV5),
// }

impl From<ConnectReturnFailCodeV3> for ConnectReturnCode {
    fn from(value: ConnectReturnFailCodeV3) -> Self {
        Self::Fail(ConnectReturnFailCode::FailV3(value))
    }
}
impl From<ConnectReturnCodeV5> for ConnectReturnCode {
    fn from(value: ConnectReturnCodeV5) -> Self {
        Self::Fail(ConnectReturnFailCode::FailV5(value))
    }
}
