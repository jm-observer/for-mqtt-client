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
    pub payload: Bytes,
}

impl Publish {
    pub fn new<S: Into<Arc<String>>, P: Into<Bytes>>(
        topic: S,
        qos: QoSWithPacketId,
        payload: P,
        retain: bool,
    ) -> Result<Publish, Error> {
        let payload = payload.into();
        let topic = topic.into();
        if payload.len() + 4 + topic.len() > 268_435_455 {
            return Err(Error::PayloadTooLong);
        };
        Ok(Publish {
            dup: false,
            qos,
            retain,
            topic: topic.into(),
            payload: payload.into(),
        })
    }

    fn len(&self) -> usize {
        let len = 2 + self.topic.len() + self.payload.len();
        if self.qos != QoSWithPacketId::AtMostOnce {
            len + 2
        } else {
            len
        }
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, Error> {
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
            payload: bytes,
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

        buffer.extend_from_slice(&self.payload);

        // TODO: Returned length is wrong in other packets. Fix it
        1 + count + len
    }
}

impl fmt::Debug for Publish {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Topic = {}, Qos = {:?}, Retain = {}, Payload Size = {}",
            self.topic,
            self.qos,
            self.retain,
            self.payload.len()
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::{Bytes, BytesMut};
    use pretty_assertions::assert_eq;

    #[test]
    fn qos1_publish_parsing_works() {
        let stream = &[
            0b0011_0010,
            11, // packet type, flags and remaining len
            0x00,
            0x03,
            b'a',
            b'/',
            b'b', // variable header. topic name = 'a/b'
            0x00,
            0x0a, // variable header. pkid = 10
            0xF1,
            0xF2,
            0xF3,
            0xF4, // publish payload
            0xDE,
            0xAD,
            0xBE,
            0xEF, // extra packets in the stream
        ];

        let mut stream = BytesMut::from(&stream[..]);
        let fixed_header = parse_fixed_header(stream.iter()).unwrap();
        let publish_bytes = stream.split_to(fixed_header.frame_length()).freeze();
        let packet = Publish::read(fixed_header, publish_bytes).unwrap();

        let payload = &[0xF1, 0xF2, 0xF3, 0xF4];
        assert_eq!(
            packet,
            Publish {
                dup: false,
                qos: QoS::AtLeastOnce,
                retain: false,
                topic: "a/b".to_owned().into(),
                pkid: 10.into(),
                payload: Bytes::from(&payload[..]),
            }
        );
    }

    #[test]
    fn qos0_publish_parsing_works() {
        let stream = &[
            0b0011_0000,
            7, // packet type, flags and remaining len
            0x00,
            0x03,
            b'a',
            b'/',
            b'b', // variable header. topic name = 'a/b'
            0x01,
            0x02, // payload
            0xDE,
            0xAD,
            0xBE,
            0xEF, // extra packets in the stream
        ];

        let mut stream = BytesMut::from(&stream[..]);
        let fixed_header = parse_fixed_header(stream.iter()).unwrap();
        let publish_bytes = stream.split_to(fixed_header.frame_length()).freeze();
        let packet = Publish::read(fixed_header, publish_bytes).unwrap();

        assert_eq!(
            packet,
            Publish {
                dup: false,
                qos: QoS::AtMostOnce,
                retain: false,
                topic: "a/b".to_owned().into(),
                pkid: 0.into(),
                payload: Bytes::from(&[0x01, 0x02][..]),
            }
        );
    }

    #[test]
    fn qos1_publish_encoding_works() {
        let publish = Publish {
            dup: false,
            qos: QoS::AtLeastOnce,
            retain: false,
            topic: "a/b".to_owned().into(),
            pkid: 10.into(),
            payload: Bytes::from(vec![0xF1, 0xF2, 0xF3, 0xF4]),
        };

        let mut buf = BytesMut::new();
        publish.write(&mut buf).unwrap();

        assert_eq!(
            buf,
            vec![
                0b0011_0010,
                11,
                0x00,
                0x03,
                b'a',
                b'/',
                b'b',
                0x00,
                0x0a,
                0xF1,
                0xF2,
                0xF3,
                0xF4
            ]
        );
    }

    #[test]
    fn qos0_publish_encoding_works() {
        let publish = Publish {
            dup: false,
            qos: QoS::AtMostOnce,
            retain: false,
            topic: "a/b".to_owned().into(),
            pkid: 0.into(),
            payload: Bytes::from(vec![0xE1, 0xE2, 0xE3, 0xE4]),
        };

        let mut buf = BytesMut::new();
        publish.write(&mut buf).unwrap();

        assert_eq!(
            buf,
            vec![
                0b0011_0000,
                9,
                0x00,
                0x03,
                b'a',
                b'/',
                b'b',
                0xE1,
                0xE2,
                0xE3,
                0xE4
            ]
        );
    }
}
