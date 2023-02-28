mod login;
mod properties;
pub(crate) mod will;
mod willproperties;

use crate::protocol::packet::{write_mqtt_string, write_remaining_length};
use crate::protocol::{len_len, MqttOptions, PacketParseError, PropertyType, Protocol};
use crate::{qos, QoS};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use login::Login;
use properties::ConnectProperties;
use std::sync::Arc;
use will::LastWill;
use willproperties::LastWillProperties;

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
    will_properties: Option<LastWillProperties>,
    connect_properties: Option<ConnectProperties>,
}

impl Connect {
    pub fn new(option: &MqttOptions) -> Result<Bytes, PacketParseError> {
        let login = match &option.credentials {
            None => None,
            Some((user, password)) => Some(Login::new(user.clone(), password.clone())),
        };
        let packet = Connect {
            protocol: option.protocol.clone(),
            keep_alive: option.keep_alive.into(),
            client_id: option.client_id.clone(),
            clean_session: option.clean_session,
            last_will: option.last_will.clone(),
            login,
            will_properties: None,
            connect_properties: None,
        };
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes)?;
        Ok(bytes.freeze())
    }

    pub fn write(&self, buffer: &mut BytesMut) -> Result<usize, PacketParseError> {
        let len = {
            let mut len = 2 + "MQTT".len() // protocol name
                + 1            // protocol version
                + 1            // connect flags
                + 2; // keep alive

            match self.protocol {
                Protocol::V4 => {}
                Protocol::V5 => {
                    if let Some(p) = &self.connect_properties {
                        let properties_len = p.len();
                        let properties_len_len = len_len(properties_len);
                        len += properties_len_len + properties_len;
                    } else {
                        // just 1 byte representing 0 len
                        len += 1;
                    }
                }
            }
            len += 2 + &self.client_id.len();

            // last will len
            if let Some(w) = &self.last_will {
                len += w.len();
            }

            // username and password len
            if let Some(l) = &self.login {
                len += l.len();
            }

            len
        };

        buffer.put_u8(0b0001_0000);
        let count = write_remaining_length(buffer, len);
        write_mqtt_string(buffer, "MQTT");

        let flags_index = 1 + count + 2 + 4 + 1;

        let mut connect_flags = 0;
        if self.clean_session {
            connect_flags |= 0x02;
        }
        match self.protocol {
            Protocol::V4 => {
                buffer.put_u8(0x04);
                buffer.put_u8(connect_flags);
                buffer.put_u16(self.keep_alive);
            }
            Protocol::V5 => {
                buffer.put_u8(0x05);
                buffer.put_u8(connect_flags);
                buffer.put_u16(self.keep_alive);

                match &self.connect_properties {
                    Some(p) => p.write(buffer)?,
                    None => {
                        write_remaining_length(buffer, 0);
                    }
                };
            }
        }

        write_mqtt_string(buffer, &&self.client_id);

        if let Some(w) = &self.last_will {
            connect_flags |= w.write(buffer)?;
        }

        if let Some(l) = &self.login {
            connect_flags |= l.write(buffer);
        }

        // update connect flags
        buffer[flags_index] = connect_flags;
        Ok(len)
    }
}
