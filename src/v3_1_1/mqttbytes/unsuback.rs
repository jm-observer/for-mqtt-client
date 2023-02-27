use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::protocol::packet::read_u16;
use crate::protocol::FixedHeader;
use crate::protocol::PacketParseError;
use crate::v3_1_1::mqttbytes::Error;
/// Acknowledgement to unsubscribe
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubAck {
    pub packet_id: u16,
}

impl UnsubAck {
    pub fn new(packet_id: u16) -> UnsubAck {
        UnsubAck { packet_id }
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        if fixed_header.remaining_len != 2 {
            return Err(PacketParseError::PayloadSizeIncorrect);
        }

        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let pkid = read_u16(&mut bytes)?;
        let unsuback = UnsubAck { packet_id: pkid };

        Ok(unsuback)
    }

    pub fn write(&self, payload: &mut BytesMut) -> Result<usize, Error> {
        payload.put_slice(&[0xB0, 0x02]);
        payload.put_u16(self.packet_id);
        Ok(4)
    }
}
