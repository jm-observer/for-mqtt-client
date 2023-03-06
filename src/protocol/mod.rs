use crate::protocol::packet::FixedHeaderError;
use crate::tasks::TaskHub;
use crate::Client;
use packet::connect::will::LastWill;
use std::sync::Arc;

pub mod packet;

#[derive(Debug, Clone)]
pub struct MqttOptions {
    /// broker address that you want to connect to
    broker_addr: String,
    /// broker port
    port: u16,
    /// keep alive time to send pingreq to broker when the connection is idle
    keep_alive: u16,
    /// clean (or) persistent session
    clean_session: bool,
    /// client identifier
    client_id: Arc<String>,
    /// username and password
    credentials: Option<(Arc<String>, Arc<String>)>,
    /// maximum incoming packet size (verifies remaining length of the packet)
    max_incoming_packet_size: usize,
    /// Maximum outgoing packet size (only verifies publish payload size)
    // TODO Verify this with all packets. This can be packet.write but message left in
    // the state might be a footgun as user has to explicitly clean it. Probably state
    // has to be moved to network
    max_outgoing_packet_size: usize,
    /// Last will that will be issued on unexpected disconnect
    last_will: Option<LastWill>,

    /// 是否自动重连
    pub(crate) auto_reconnect: bool,
}

impl MqttOptions {
    pub fn new<S: Into<Arc<String>>, T: Into<String>>(id: S, host: T, port: u16) -> MqttOptions {
        let id = id.into();
        if id.starts_with(' ') || id.is_empty() {
            panic!("Invalid client id");
        }

        MqttOptions {
            broker_addr: host.into(),
            port,
            keep_alive: 60,
            clean_session: true,
            client_id: id,
            credentials: None,
            max_incoming_packet_size: 10 * 1024,
            max_outgoing_packet_size: 10 * 1024,
            last_will: None,
            auto_reconnect: false,
        }
    }
    /// 设置为自动重连，会默认设置clean_session=false
    pub fn auto_reconnect(&mut self) -> &mut Self {
        self.auto_reconnect = true;
        self.set_clean_session(false);
        self
    }

    /// Broker address
    pub fn broker_address(&self) -> (String, u16) {
        (self.broker_addr.clone(), self.port)
    }

    pub fn set_last_will(&mut self, will: LastWill) -> &mut Self {
        self.last_will = Some(will);
        self
    }

    pub fn last_will(&self) -> Option<LastWill> {
        self.last_will.clone()
    }

    /// Set number of seconds after which client should ping the broker
    /// if there is no other data exchange
    pub fn set_keep_alive(&mut self, duration: u16) -> &mut Self {
        assert!(duration >= 5, "Keep alives should be >= 5 secs");
        self.keep_alive = duration;
        self
    }

    /// Keep alive time
    pub fn keep_alive(&self) -> u16 {
        self.keep_alive
    }

    /// Client identifier
    pub fn client_id(&self) -> Arc<String> {
        self.client_id.clone()
    }

    /// Set packet size limit for outgoing an incoming packets
    pub fn set_max_packet_size(&mut self, incoming: usize, outgoing: usize) -> &mut Self {
        self.max_incoming_packet_size = incoming;
        self.max_outgoing_packet_size = outgoing;
        self
    }

    /// Maximum packet size
    pub fn max_packet_size(&self) -> usize {
        self.max_incoming_packet_size
    }

    /// `clean_session = true` removes all the state from queues & instructs the broker
    /// to clean all the client state when client disconnects.
    ///
    /// When set `false`, broker will hold the client state and performs pending
    /// operations on the client when reconnection with same `client_id`
    /// happens. Local queue state is also held to retransmit packets after reconnection.
    pub fn set_clean_session(&mut self, clean_session: bool) -> &mut Self {
        self.clean_session = clean_session;
        self
    }

    /// Clean session
    pub fn clean_session(&self) -> bool {
        self.clean_session
    }

    /// Username and password
    pub fn set_credentials<U: Into<Arc<String>>, P1: Into<Arc<String>>>(
        &mut self,
        username: U,
        password: P1,
    ) -> &mut Self {
        self.credentials = Some((username.into(), password.into()));
        self
    }

    /// Security options
    pub fn credentials(&self) -> Option<(Arc<String>, Arc<String>)> {
        self.credentials.clone()
    }

    pub async fn connect_to_v4(self) -> Client {
        TaskHub::connect(self, Protocol::V4).await
    }
    pub async fn connect_to_v5(self) -> Client {
        TaskHub::connect(self, Protocol::V5).await
    }
}

/// Protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    V4,
    V5,
}

impl Protocol {
    pub fn is_v5(&self) -> bool {
        *self == Protocol::V5
    }
}

/// Packet type from a byte
///
/// ```text
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
    pub(crate) byte1: u8,
    /// Length of fixed header. Byte 1 + (1..4) bytes. So fixed header
    /// len can vary from 2 bytes to 5 bytes
    /// 1..4 bytes are variable length encoded to represent remaining length
    pub(crate) fixed_header_len: usize,
    /// Remaining length of the packet. Doesn't include fixed header bytes
    /// Represents variable header + payload size
    pub(crate) remaining_len: usize,
}

impl FixedHeader {
    pub fn new(byte1: u8, remaining_len_len: usize, remaining_len: usize) -> FixedHeader {
        FixedHeader {
            byte1,
            fixed_header_len: remaining_len_len + 1,
            remaining_len,
        }
    }

    pub fn packet_type(&self) -> Result<PacketType, PacketParseError> {
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
            // todo
            _ => Err(PacketParseError::InvalidPacketType(num)),
        }
    }

    /// Returns the size of full packet (fixed header + variable header + payload)
    /// Fixed header is enough to get the size of a frame in the stream
    pub fn frame_length(&self) -> usize {
        self.fixed_header_len + self.remaining_len
    }

    pub fn remaining_len(&self) -> usize {
        self.remaining_len
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

impl From<FixedHeaderError> for PacketParseError {
    fn from(value: FixedHeaderError) -> Self {
        match value {
            FixedHeaderError::InsufficientBytes(len) => Self::InsufficientBytes(len),
            FixedHeaderError::MalformedRemainingLength => Self::MalformedRemainingLength,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyType {
    PayloadFormatIndicator = 1,
    MessageExpiryInterval = 2,
    ContentType = 3,
    ResponseTopic = 8,
    CorrelationData = 9,
    SubscriptionIdentifier = 11,
    SessionExpiryInterval = 17,
    AssignedClientIdentifier = 18,
    ServerKeepAlive = 19,
    AuthenticationMethod = 21,
    AuthenticationData = 22,
    RequestProblemInformation = 23,
    WillDelayInterval = 24,
    RequestResponseInformation = 25,
    ResponseInformation = 26,
    ServerReference = 28,
    ReasonString = 31,
    ReceiveMaximum = 33,
    TopicAliasMaximum = 34,
    TopicAlias = 35,
    MaximumQos = 36,
    RetainAvailable = 37,
    UserProperty = 38,
    MaximumPacketSize = 39,
    WildcardSubscriptionAvailable = 40,
    SubscriptionIdentifierAvailable = 41,
    SharedSubscriptionAvailable = 42,
}

/// Error during serialization and deserialization
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PacketParseError {
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
    #[error("At least {0} more bytes required to frame packet")]
    InsufficientBytes(usize),
    #[error("Malformed remaining length")]
    MalformedRemainingLength,
}

fn property(num: u8) -> Result<PropertyType, PacketParseError> {
    let property = match num {
        1 => PropertyType::PayloadFormatIndicator,
        2 => PropertyType::MessageExpiryInterval,
        3 => PropertyType::ContentType,
        8 => PropertyType::ResponseTopic,
        9 => PropertyType::CorrelationData,
        11 => PropertyType::SubscriptionIdentifier,
        17 => PropertyType::SessionExpiryInterval,
        18 => PropertyType::AssignedClientIdentifier,
        19 => PropertyType::ServerKeepAlive,
        21 => PropertyType::AuthenticationMethod,
        22 => PropertyType::AuthenticationData,
        23 => PropertyType::RequestProblemInformation,
        24 => PropertyType::WillDelayInterval,
        25 => PropertyType::RequestResponseInformation,
        26 => PropertyType::ResponseInformation,
        28 => PropertyType::ServerReference,
        31 => PropertyType::ReasonString,
        33 => PropertyType::ReceiveMaximum,
        34 => PropertyType::TopicAliasMaximum,
        35 => PropertyType::TopicAlias,
        36 => PropertyType::MaximumQos,
        37 => PropertyType::RetainAvailable,
        38 => PropertyType::UserProperty,
        39 => PropertyType::MaximumPacketSize,
        40 => PropertyType::WildcardSubscriptionAvailable,
        41 => PropertyType::SubscriptionIdentifierAvailable,
        42 => PropertyType::SharedSubscriptionAvailable,
        num => return Err(PacketParseError::InvalidPropertyType(num)),
    };

    Ok(property)
}

/// Checks if the stream has enough bytes to frame a packet and returns fixed header
/// only if a packet can be framed with existing bytes in the `stream`.
/// The passed stream doesn't modify parent stream's cursor. If this function
/// returned an error, next `check` on the same parent stream is forced start
/// with cursor at 0 again (Iter is owned. Only Iter's cursor is changed internally)
// pub fn check(stream: Iter<u8>, max_packet_size: usize) -> Result<FixedHeader, PacketParseError> {
//     // Create fixed header if there are enough bytes in the stream
//     // to frame full packet
//     let stream_len = stream.len();
//     let fixed_header = parse_fixed_header(stream)?;
//
//     // Don't let rogue connections attack with huge payloads.
//     // Disconnect them before reading all that data
//     if fixed_header.remaining_len > max_packet_size {
//         return Err(PacketParseError::PayloadSizeLimitExceeded(
//             fixed_header.remaining_len,
//         ));
//     }
//
//     // If the current call fails due to insufficient bytes in the stream,
//     // after calculating remaining length, we extend the stream
//     let frame_length = fixed_header.frame_length();
//     if stream_len < frame_length {
//         return Err(PacketParseError::InsufficientBytes(
//             frame_length - stream_len,
//         ));
//     }
//
//     Ok(fixed_header)
// }

/// Parses fixed header
// fn parse_fixed_header(mut stream: Iter<u8>) -> Result<FixedHeader, PacketParseError> {
//     // At least 2 bytes are necessary to frame a packet
//     let stream_len = stream.len();
//     if stream_len < 2 {
//         return Err(PacketParseError::InsufficientBytes(2 - stream_len));
//     }
//
//     let byte1 = stream.next().unwrap();
//     let (len_len, len) = length(stream)?;
//
//     Ok(FixedHeader::new(*byte1, len_len, len))
// }

/// Return number of remaining length bytes required for encoding length
fn len_len(len: usize) -> usize {
    if len >= 2_097_152 {
        4
    } else if len >= 16_384 {
        3
    } else if len >= 128 {
        2
    } else {
        1
    }
}

// fn len_variable_integer(stream: Iter<u8>) -> Result<(usize, u32), PacketParseError> {
//     let mut len = 0;
//     let mut val = [0u8; 4];
//     for byte in stream {
//         val[len] = *byte;
//         len += 1;
//         if len > 4 {
//             return Err(PacketParseError::MalformedRemainingLength);
//         }
//         if *byte >= 0x80 {
//             continue;
//         }
//     }
//     Ok((len, u32::from_be_bytes(val)))
// }
