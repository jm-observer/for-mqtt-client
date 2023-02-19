use super::*;
use crate::QoSWithPacketId;
use bytes::{Buf, Bytes};
use std::sync::Arc;

/// Publish packet
#[derive(Clone, PartialEq, Eq)]
pub struct Publish {
    pub dup: bool,
    pub qos: QoSWithPacketId,
    pub retain: bool,
    pub topic: Arc<String>,
    pub payload: Arc<Bytes>,
}

impl Publish {
    pub fn new<S: Into<Arc<String>>, P: Into<Arc<Bytes>>>(
        topic: S,
        qos: QoSWithPacketId,
        payload: P,
        retain: bool,
    ) -> Publish {
        let payload = payload.into();
        let topic = topic.into();

        Publish {
            dup: false,
            qos,
            retain,
            topic: topic.into(),
            payload: payload.into(),
        }
    }

    fn len(&self) -> usize {
        let len = 2 + self.topic.len() + self.payload.len();
        if self.qos != QoSWithPacketId::AtMostOnce {
            len + 2
        } else {
            len
        }
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        let qos = qos((fixed_header.byte1 & 0b0110) >> 1)?;
        let dup = (fixed_header.byte1 & 0b1000) != 0;
        let retain = (fixed_header.byte1 & 0b0001) != 0;

        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let topic = read_mqtt_string(&mut bytes)?.into();

        // Packet identifier exists where QoS > 0
        let qos = match qos {
            QoS::AtMostOnce => QoSWithPacketId::AtMostOnce,
            QoS::AtLeastOnce => QoSWithPacketId::AtLeastOnce(read_u16(&mut bytes)?),
            QoS::ExactlyOnce => QoSWithPacketId::ExactlyOnce(read_u16(&mut bytes)?),
        };

        let publish = Publish {
            dup,
            retain,
            qos,
            topic,
            payload: Arc::new(bytes.into()),
        };

        Ok(publish)
    }

    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        let len = self.len();

        let dup = self.dup as u8;
        let qos = self.qos.qos();
        let retain = self.retain as u8;
        buffer.put_u8(0b0011_0000 | retain | qos << 1 | dup << 3);

        let count = write_remaining_length(buffer, len);
        write_mqtt_string(buffer, self.topic.as_str());

        if let Some(pkid) = self.qos.packet_id() {
            buffer.put_u16(pkid);
        }

        buffer.extend_from_slice(&self.payload.as_ref());

        // TODO: Returned length is wrong in other packets. Fix it
        1 + count + len
    }
}

impl fmt::Debug for Publish {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Topic = {}, Qos = {:?}, Retain = {}, Payload = {:?}",
            self.topic, self.qos, self.retain, self.payload
        )
    }
}
