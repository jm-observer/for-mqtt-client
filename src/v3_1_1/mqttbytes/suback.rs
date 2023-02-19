use super::*;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::convert::{TryFrom, TryInto};

/// Acknowledgement to subscribe
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubAck {
    pub packet_id: u16,
    pub return_codes: Vec<SubscribeReasonCode>,
}

impl SubAck {
    pub fn new(packet_id: u16, return_codes: Vec<SubscribeReasonCode>) -> SubAck {
        SubAck {
            packet_id,
            return_codes,
        }
    }

    fn len(&self) -> usize {
        2 + self.return_codes.len()
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let pkid = read_u16(&mut bytes)?;

        if !bytes.has_remaining() {
            return Err(PacketParseError::MalformedPacket);
        }

        let mut return_codes = Vec::new();
        while bytes.has_remaining() {
            let return_code = read_u8(&mut bytes)?;
            return_codes.push(return_code.try_into()?);
        }

        let suback = SubAck {
            packet_id: pkid,
            return_codes,
        };
        Ok(suback)
    }

    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        buffer.put_u8(0x90);
        let remaining_len = self.len();
        let remaining_len_bytes = write_remaining_length(buffer, remaining_len);

        buffer.put_u16(self.packet_id);
        let p: Vec<u8> = self
            .return_codes
            .iter()
            .map(|&code| match code {
                SubscribeReasonCode::Success(qos) => qos as u8,
                SubscribeReasonCode::Failure => 0x80,
            })
            .collect();
        buffer.extend_from_slice(&p);
        1 + remaining_len_bytes + remaining_len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscribeReasonCode {
    Success(QoS),
    Failure,
}

impl TryFrom<u8> for SubscribeReasonCode {
    type Error = PacketParseError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let v = match value {
            0 => SubscribeReasonCode::Success(QoS::AtMostOnce),
            1 => SubscribeReasonCode::Success(QoS::AtLeastOnce),
            2 => SubscribeReasonCode::Success(QoS::ExactlyOnce),
            128 => SubscribeReasonCode::Failure,
            v => return Err(PacketParseError::InvalidSubscribeReasonCode(v)),
        };

        Ok(v)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BytesMut;
    use pretty_assertions::assert_eq;

    #[test]
    fn suback_parsing_works() -> anyhow::Result<()> {
        let stream = vec![
            0x90, 4, // packet type, flags and remaining len
            0x00, 0x0F, // variable header. pkid = 15
            0x01, 0x80, // payload. return codes [success qos1, failure]
            0xDE, 0xAD, 0xBE, 0xEF, // extra packets in the stream
        ];

        let mut stream = BytesMut::from(&stream[..]);
        let fixed_header = parse_fixed_header(stream.iter())?;
        let ack_bytes = stream.split_to(fixed_header.frame_length()).freeze();
        let packet = SubAck::read(fixed_header, ack_bytes)?;

        assert_eq!(
            packet,
            SubAck {
                packet_id: 15,
                return_codes: vec![
                    SubscribeReasonCode::Success(QoS::AtLeastOnce),
                    SubscribeReasonCode::Failure,
                ],
            }
        );
        Ok(())
    }
}
