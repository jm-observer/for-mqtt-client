use super::*;
use crate::protocol::packet::{read_mqtt_string, write_mqtt_string};
use crate::protocol::PacketParseError;

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

    pub fn read(connect_flags: u8, bytes: &mut Bytes) -> Result<Option<Login>, PacketParseError> {
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

    pub fn len(&self) -> usize {
        let mut len = 0;

        if !self.username.is_empty() {
            len += 2 + self.username.len();
        }

        if !self.password.is_empty() {
            len += 2 + self.password.len();
        }

        len
    }

    pub fn write(&self, buffer: &mut BytesMut) -> u8 {
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
