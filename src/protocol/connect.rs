use crate::protocol::PacketParseError;
use crate::protocol::Protocol;
use crate::{qos, QoS};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::sync::Arc;

/// Connection packet initiated by the client
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    /// Mqtt protocol version
    pub protocol: Protocol,
    /// Mqtt keep alive time
    pub keep_alive: u16,
    /// Client Id
    pub client_id: Arc<String>,
    /// Clean session. Asks the broker to clear previous state
    pub clean_session: bool,
    /// Will that broker needs to publish when the client disconnects
    pub last_will: Option<LastWill>,
    /// Login credentials
    pub login: Option<Login>,
}

impl Connect {
    #[allow(clippy::type_complexity)]
    pub fn read(
        fixed_header: FixedHeader,
        mut bytes: Bytes,
    ) -> Result<
        (
            Connect,
            Option<ConnectProperties>,
            Option<LastWill>,
            Option<LastWillProperties>,
            Option<Login>,
        ),
        Error,
    > {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);

        // Variable header
        let protocol_name = read_mqtt_string(&mut bytes)?;
        let protocol_level = read_u8(&mut bytes)?;
        if protocol_name != "MQTT" {
            return Err(Error::InvalidProtocol);
        }

        if protocol_level != 5 {
            return Err(Error::InvalidProtocolLevel(protocol_level));
        }

        let connect_flags = read_u8(&mut bytes)?;
        let clean_session = (connect_flags & 0b10) != 0;
        let keep_alive = read_u16(&mut bytes)?;

        let properties = properties::read(&mut bytes)?;

        let client_id = read_mqtt_string(&mut bytes)?;
        let (will, willproperties) = will::read(connect_flags, &mut bytes)?;
        let login = login::read(connect_flags, &mut bytes)?;

        let connect = Connect {
            keep_alive,
            client_id,
            clean_session,
        };

        Ok((connect, properties, will, willproperties, login))
    }

    pub fn write(
        connect: &Connect,
        will: &Option<LastWill>,
        will_properties: &Option<LastWillProperties>,
        l: &Option<Login>,
        properties: &Option<ConnectProperties>,
        buffer: &mut BytesMut,
    ) -> Result<usize, Error> {
        let len = {
            let mut len = 2 + "MQTT".len() // protocol name
                + 1            // protocol version
                + 1            // connect flags
                + 2; // keep alive

            if let Some(p) = properties {
                let properties_len = properties::len(p);
                let properties_len_len = len_len(properties_len);
                len += properties_len_len + properties_len;
            } else {
                // just 1 byte representing 0 len
                len += 1;
            }

            len += 2 + connect.client_id.len();

            // last will len
            if let Some(w) = will {
                len += will::len(w, will_properties);
            }

            // username and password len
            if let Some(l) = l {
                len += login::len(l);
            }

            len
        };

        buffer.put_u8(0b0001_0000);
        let count = write_remaining_length(buffer, len)?;
        write_mqtt_string(buffer, "MQTT");

        buffer.put_u8(0x05);
        let flags_index = 1 + count + 2 + 4 + 1;

        let mut connect_flags = 0;
        if connect.clean_session {
            connect_flags |= 0x02;
        }

        buffer.put_u8(connect_flags);
        buffer.put_u16(connect.keep_alive);

        match properties {
            Some(p) => properties::write(p, buffer)?,
            None => {
                write_remaining_length(buffer, 0)?;
            }
        };

        write_mqtt_string(buffer, &connect.client_id);

        if let Some(w) = will {
            connect_flags |= will::write(w, will_properties, buffer)?;
        }

        if let Some(l) = l {
            connect_flags |= login::write(l, buffer);
        }

        // update connect flags
        buffer[flags_index] = connect_flags;
        Ok(len)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectProperties {
    /// Expiry interval property after loosing connection
    pub session_expiry_interval: Option<u32>,
    /// Maximum simultaneous packets
    pub receive_maximum: Option<u16>,
    /// Maximum packet size
    pub max_packet_size: Option<u32>,
    /// Maximum mapping integer for a topic
    pub topic_alias_max: Option<u16>,
    pub request_response_info: Option<u8>,
    pub request_problem_info: Option<u8>,
    /// List of user properties
    pub user_properties: Vec<(String, String)>,
    /// Method of authentication
    pub authentication_method: Option<String>,
    /// Authentication data
    pub authentication_data: Option<Bytes>,
}

/// LastWill that broker forwards on behalf of the client
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub topic: String,
    pub message: Bytes,
    pub qos: QoS,
    pub retain: bool,
}

impl LastWill {
    pub fn new(
        topic: impl Into<String>,
        payload: impl Into<Vec<u8>>,
        qos: QoS,
        retain: bool,
    ) -> LastWill {
        LastWill {
            topic: topic.into(),
            message: Bytes::from(payload.into()),
            qos,
            retain,
        }
    }
    pub fn len(will: &LastWill, properties: &Option<LastWillProperties>) -> usize {
        let mut len = 0;

        if let Some(p) = properties {
            let properties_len = willproperties::len(p);
            let properties_len_len = len_len(properties_len);
            len += properties_len_len + properties_len;
        } else {
            // just 1 byte representing 0 len
            len += 1;
        }

        len += 2 + will.topic.len() + 2 + will.message.len();
        len
    }

    pub fn read(
        connect_flags: u8,
        bytes: &mut Bytes,
    ) -> Result<(Option<LastWill>, Option<LastWillProperties>), Error> {
        let o = match connect_flags & 0b100 {
            0 if (connect_flags & 0b0011_1000) != 0 => {
                return Err(Error::IncorrectPacketFormat);
            }
            0 => (None, None),
            _ => {
                // Properties in variable header
                let properties = willproperties::read(bytes)?;

                let will_topic = read_mqtt_bytes(bytes)?;
                let will_message = read_mqtt_bytes(bytes)?;
                let qos_num = (connect_flags & 0b11000) >> 3;
                let will_qos = qos(qos_num).ok_or(Error::InvalidQoS(qos_num))?;
                let will = Some(LastWill {
                    topic: will_topic,
                    message: will_message,
                    qos: will_qos,
                    retain: (connect_flags & 0b0010_0000) != 0,
                });

                (will, properties)
            }
        };

        Ok(o)
    }

    pub fn write(
        will: &LastWill,
        properties: &Option<LastWillProperties>,
        buffer: &mut BytesMut,
    ) -> Result<u8, Error> {
        let mut connect_flags = 0;

        connect_flags |= 0x04 | (will.qos as u8) << 3;
        if will.retain {
            connect_flags |= 0x20;
        }

        if let Some(p) = properties {
            willproperties::write(p, buffer)?;
        } else {
            write_remaining_length(buffer, 0)?;
        }

        write_mqtt_bytes(buffer, &will.topic);
        write_mqtt_bytes(buffer, &will.message);
        Ok(connect_flags)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWillProperties {
    pub delay_interval: Option<u32>,
    pub payload_format_indicator: Option<u8>,
    pub message_expiry_interval: Option<u32>,
    pub content_type: Option<String>,
    pub response_topic: Option<String>,
    pub correlation_data: Option<Bytes>,
    pub user_properties: Vec<(String, String)>,
}

impl LastWillProperties {
    pub fn len(properties: &LastWillProperties) -> usize {
        let mut len = 0;

        if properties.delay_interval.is_some() {
            len += 1 + 4;
        }

        if properties.payload_format_indicator.is_some() {
            len += 1 + 1;
        }

        if properties.message_expiry_interval.is_some() {
            len += 1 + 4;
        }

        if let Some(typ) = &properties.content_type {
            len += 1 + 2 + typ.len()
        }

        if let Some(topic) = &properties.response_topic {
            len += 1 + 2 + topic.len()
        }

        if let Some(data) = &properties.correlation_data {
            len += 1 + 2 + data.len()
        }

        for (key, value) in properties.user_properties.iter() {
            len += 1 + 2 + key.len() + 2 + value.len();
        }

        len
    }

    pub fn read(bytes: &mut Bytes) -> Result<Option<LastWillProperties>, Error> {
        let mut delay_interval = None;
        let mut payload_format_indicator = None;
        let mut message_expiry_interval = None;
        let mut content_type = None;
        let mut response_topic = None;
        let mut correlation_data = None;
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
                PropertyType::WillDelayInterval => {
                    delay_interval = Some(read_u32(bytes)?);
                    cursor += 4;
                }
                PropertyType::PayloadFormatIndicator => {
                    payload_format_indicator = Some(read_u8(bytes)?);
                    cursor += 1;
                }
                PropertyType::MessageExpiryInterval => {
                    message_expiry_interval = Some(read_u32(bytes)?);
                    cursor += 4;
                }
                PropertyType::ContentType => {
                    let typ = read_mqtt_string(bytes)?;
                    cursor += 2 + typ.len();
                    content_type = Some(typ);
                }
                PropertyType::ResponseTopic => {
                    let topic = read_mqtt_string(bytes)?;
                    cursor += 2 + topic.len();
                    response_topic = Some(topic);
                }
                PropertyType::CorrelationData => {
                    let data = read_mqtt_bytes(bytes)?;
                    cursor += 2 + data.len();
                    correlation_data = Some(data);
                }
                PropertyType::UserProperty => {
                    let key = read_mqtt_string(bytes)?;
                    let value = read_mqtt_string(bytes)?;
                    cursor += 2 + key.len() + 2 + value.len();
                    user_properties.push((key, value));
                }
                _ => return Err(Error::InvalidPropertyType(prop)),
            }
        }

        Ok(Some(LastWillProperties {
            delay_interval,
            payload_format_indicator,
            message_expiry_interval,
            content_type,
            response_topic,
            correlation_data,
            user_properties,
        }))
    }

    pub fn write(properties: &LastWillProperties, buffer: &mut BytesMut) -> Result<(), Error> {
        let len = len(properties);
        write_remaining_length(buffer, len)?;

        if let Some(delay_interval) = properties.delay_interval {
            buffer.put_u8(PropertyType::WillDelayInterval as u8);
            buffer.put_u32(delay_interval);
        }

        if let Some(payload_format_indicator) = properties.payload_format_indicator {
            buffer.put_u8(PropertyType::PayloadFormatIndicator as u8);
            buffer.put_u8(payload_format_indicator);
        }

        if let Some(message_expiry_interval) = properties.message_expiry_interval {
            buffer.put_u8(PropertyType::MessageExpiryInterval as u8);
            buffer.put_u32(message_expiry_interval);
        }

        if let Some(typ) = &properties.content_type {
            buffer.put_u8(PropertyType::ContentType as u8);
            write_mqtt_string(buffer, typ);
        }

        if let Some(topic) = &properties.response_topic {
            buffer.put_u8(PropertyType::ResponseTopic as u8);
            write_mqtt_string(buffer, topic);
        }

        if let Some(data) = &properties.correlation_data {
            buffer.put_u8(PropertyType::CorrelationData as u8);
            write_mqtt_bytes(buffer, data);
        }

        for (key, value) in properties.user_properties.iter() {
            buffer.put_u8(PropertyType::UserProperty as u8);
            write_mqtt_string(buffer, key);
            write_mqtt_string(buffer, value);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Login {
    pub username: Arc<String>,
    pub password: Arc<String>,
}

impl Login {
    pub fn new<U: Into<Arc<String>>, P: Into<Arc<String>>>(u: U, p: P) -> Login {
        Login {
            username: u.into(),
            password: p.into(),
        }
    }

    fn read(connect_flags: u8, bytes: &mut Bytes) -> Result<Option<Login>, PacketParseError> {
        let username = Arc::new(match connect_flags & 0b1000_0000 {
            0 => String::new(),
            _ => read_mqtt_string(bytes)?,
        });

        let password = Arc::new(match connect_flags & 0b0100_0000 {
            0 => String::new(),
            _ => read_mqtt_string(bytes)?,
        });

        if username.is_empty() && password.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Login { username, password }))
        }
    }

    fn len(&self) -> usize {
        let mut len = 0;

        if !self.username.is_empty() {
            len += 2 + self.username.len();
        }

        if !self.password.is_empty() {
            len += 2 + self.password.len();
        }

        len
    }

    fn write(&self, buffer: &mut BytesMut) -> u8 {
        let mut connect_flags = 0;
        if !self.username.is_empty() {
            connect_flags |= 0x80;
            write_mqtt_string(buffer, &self.username);
        }

        if !self.password.is_empty() {
            connect_flags |= 0x40;
            write_mqtt_string(buffer, &self.password);
        }

        connect_flags
    }

    pub fn validate(&self, username: &str, password: &str) -> bool {
        (self.username.as_str() == username) && (self.password.as_str() == password)
    }
}
