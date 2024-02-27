use super::*;
use crate::protocol::{
    len_len,
    packet::{
        read_mqtt_bytes, read_mqtt_string, write_mqtt_bytes,
        write_mqtt_string, write_remaining_length
    },
    PacketParseError
};

/// LastWill that broker forwards on behalf of the client
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWill {
    pub topic:      String,
    pub message:    Bytes,
    pub qos:        QoS,
    pub retain:     bool,
    pub properties: Option<LastWillProperties>
}

impl LastWill {
    pub fn new(
        topic: impl Into<String>,
        payload: impl Into<Vec<u8>>,
        qos: QoS,
        retain: bool
    ) -> LastWill {
        LastWill {
            topic: topic.into(),
            message: Bytes::from(payload.into()),
            qos,
            retain,
            properties: None
        }
    }

    pub fn len(&self) -> usize {
        let mut len = 0;

        if let Some(p) = &self.properties {
            let properties_len = p.len();
            let properties_len_len = len_len(properties_len);
            len += properties_len_len + properties_len;
        } else {
            // just 1 byte representing 0 len
            len += 1;
        }

        len += 2 + self.topic.len() + 2 + self.message.len();
        len
    }

    pub fn read(
        connect_flags: u8,
        bytes: &mut Bytes
    ) -> Result<Option<LastWill>, PacketParseError> {
        let o = match connect_flags & 0b100 {
            0 if (connect_flags & 0b0011_1000) != 0 => {
                return Err(PacketParseError::IncorrectPacketFormat);
            },
            0 => None,
            _ => {
                // Properties in variable header
                let properties = LastWillProperties::read(bytes)?;

                let will_topic = read_mqtt_string(bytes)?;
                let will_message = read_mqtt_bytes(bytes)?;
                let qos_num = (connect_flags & 0b11000) >> 3;
                let will_qos = qos(qos_num)?;
                Some(LastWill {
                    topic: will_topic,
                    message: will_message,
                    qos: will_qos,
                    retain: (connect_flags & 0b0010_0000) != 0,
                    properties
                })
            }
        };

        Ok(o)
    }

    pub fn write(
        &self,
        buffer: &mut BytesMut
    ) -> Result<u8, PacketParseError> {
        let mut connect_flags = 0;

        connect_flags |= 0x04 | (self.qos as u8) << 3;
        if self.retain {
            connect_flags |= 0x20;
        }

        if let Some(p) = &self.properties {
            p.write(buffer)?;
        } else {
            write_remaining_length(buffer, 0);
        }

        write_mqtt_string(buffer, &self.topic);
        write_mqtt_bytes(buffer, &self.message);
        Ok(connect_flags)
    }
}
