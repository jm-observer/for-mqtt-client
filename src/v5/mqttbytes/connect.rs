use super::Connect;

use super::*;
use bytes::{Buf, Bytes};

mod will {
    use super::*;

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

mod willproperties {
    use super::*;
}

mod login {
    use super::*;

    // pub fn new<U: Into<String>, P: Into<String>>(u: U, p: P) -> Login {
    //     Login {
    //         username: u.into(),
    //         password: p.into(),
    //     }
    // }

    pub fn read(connect_flags: u8, bytes: &mut Bytes) -> Result<Option<Login>, Error> {
        let username = match connect_flags & 0b1000_0000 {
            0 => String::new(),
            _ => read_mqtt_string(bytes)?,
        };

        let password = match connect_flags & 0b0100_0000 {
            0 => String::new(),
            _ => read_mqtt_string(bytes)?,
        };

        if username.is_empty() && password.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Login { username, password }))
        }
    }

    pub fn len(login: &Login) -> usize {
        let mut len = 0;

        if !login.username.is_empty() {
            len += 2 + login.username.len();
        }

        if !login.password.is_empty() {
            len += 2 + login.password.len();
        }

        len
    }

    pub fn write(login: &Login, buffer: &mut BytesMut) -> u8 {
        let mut connect_flags = 0;
        if !login.username.is_empty() {
            connect_flags |= 0x80;
            write_mqtt_string(buffer, &login.username);
        }

        if !login.password.is_empty() {
            connect_flags |= 0x40;
            write_mqtt_string(buffer, &login.password);
        }

        connect_flags
    }
}

mod properties {
    use super::*;

    pub fn read(bytes: &mut Bytes) -> Result<Option<ConnectProperties>, Error> {
        let mut session_expiry_interval = None;
        let mut receive_maximum = None;
        let mut max_packet_size = None;
        let mut topic_alias_max = None;
        let mut request_response_info = None;
        let mut request_problem_info = None;
        let mut user_properties = Vec::new();
        let mut authentication_method = None;
        let mut authentication_data = None;

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
                PropertyType::SessionExpiryInterval => {
                    session_expiry_interval = Some(read_u32(bytes)?);
                    cursor += 4;
                }
                PropertyType::ReceiveMaximum => {
                    receive_maximum = Some(read_u16(bytes)?);
                    cursor += 2;
                }
                PropertyType::MaximumPacketSize => {
                    max_packet_size = Some(read_u32(bytes)?);
                    cursor += 4;
                }
                PropertyType::TopicAliasMaximum => {
                    topic_alias_max = Some(read_u16(bytes)?);
                    cursor += 2;
                }
                PropertyType::RequestResponseInformation => {
                    request_response_info = Some(read_u8(bytes)?);
                    cursor += 1;
                }
                PropertyType::RequestProblemInformation => {
                    request_problem_info = Some(read_u8(bytes)?);
                    cursor += 1;
                }
                PropertyType::UserProperty => {
                    let key = read_mqtt_string(bytes)?;
                    let value = read_mqtt_string(bytes)?;
                    cursor += 2 + key.len() + 2 + value.len();
                    user_properties.push((key, value));
                }
                PropertyType::AuthenticationMethod => {
                    let method = read_mqtt_string(bytes)?;
                    cursor += 2 + method.len();
                    authentication_method = Some(method);
                }
                PropertyType::AuthenticationData => {
                    let data = read_mqtt_bytes(bytes)?;
                    cursor += 2 + data.len();
                    authentication_data = Some(data);
                }
                _ => return Err(Error::InvalidPropertyType(prop)),
            }
        }

        Ok(Some(ConnectProperties {
            session_expiry_interval,
            receive_maximum,
            max_packet_size,
            topic_alias_max,
            request_response_info,
            request_problem_info,
            user_properties,
            authentication_method,
            authentication_data,
        }))
    }

    pub fn len(properties: &ConnectProperties) -> usize {
        let mut len = 0;

        if properties.session_expiry_interval.is_some() {
            len += 1 + 4;
        }

        if properties.receive_maximum.is_some() {
            len += 1 + 2;
        }

        if properties.max_packet_size.is_some() {
            len += 1 + 4;
        }

        if properties.topic_alias_max.is_some() {
            len += 1 + 2;
        }

        if properties.request_response_info.is_some() {
            len += 1 + 1;
        }

        if properties.request_problem_info.is_some() {
            len += 1 + 1;
        }

        for (key, value) in properties.user_properties.iter() {
            len += 1 + 2 + key.len() + 2 + value.len();
        }

        if let Some(authentication_method) = &properties.authentication_method {
            len += 1 + 2 + authentication_method.len();
        }

        if let Some(authentication_data) = &properties.authentication_data {
            len += 1 + 2 + authentication_data.len();
        }

        len
    }

    pub fn write(properties: &ConnectProperties, buffer: &mut BytesMut) -> Result<(), Error> {
        let len = len(properties);
        write_remaining_length(buffer, len)?;

        if let Some(session_expiry_interval) = properties.session_expiry_interval {
            buffer.put_u8(PropertyType::SessionExpiryInterval as u8);
            buffer.put_u32(session_expiry_interval);
        }

        if let Some(receive_maximum) = properties.receive_maximum {
            buffer.put_u8(PropertyType::ReceiveMaximum as u8);
            buffer.put_u16(receive_maximum);
        }

        if let Some(max_packet_size) = properties.max_packet_size {
            buffer.put_u8(PropertyType::MaximumPacketSize as u8);
            buffer.put_u32(max_packet_size);
        }

        if let Some(topic_alias_max) = properties.topic_alias_max {
            buffer.put_u8(PropertyType::TopicAliasMaximum as u8);
            buffer.put_u16(topic_alias_max);
        }

        if let Some(request_response_info) = properties.request_response_info {
            buffer.put_u8(PropertyType::RequestResponseInformation as u8);
            buffer.put_u8(request_response_info);
        }

        if let Some(request_problem_info) = properties.request_problem_info {
            buffer.put_u8(PropertyType::RequestProblemInformation as u8);
            buffer.put_u8(request_problem_info);
        }

        for (key, value) in properties.user_properties.iter() {
            buffer.put_u8(PropertyType::UserProperty as u8);
            write_mqtt_string(buffer, key);
            write_mqtt_string(buffer, value);
        }

        if let Some(authentication_method) = &properties.authentication_method {
            buffer.put_u8(PropertyType::AuthenticationMethod as u8);
            write_mqtt_string(buffer, authentication_method);
        }

        if let Some(authentication_data) = &properties.authentication_data {
            buffer.put_u8(PropertyType::AuthenticationData as u8);
            write_mqtt_bytes(buffer, authentication_data);
        }

        Ok(())
    }
}
