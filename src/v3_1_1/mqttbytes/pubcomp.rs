use super::*;
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::sync::Arc;
/// QoS2 Assured publish complete, in response to PUBREL packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubComp {
    pub packet_id: u16,
}

impl PubComp {
    pub fn data(packet_id: u16) -> Arc<Bytes> {
        let packet = PubComp { packet_id };
        let mut bytes = BytesMut::new();
        packet.write(&mut bytes);
        bytes.freeze().into()
    }
    fn len(&self) -> usize {
        // pkid
        2
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);
        let pkid = read_u16(&mut bytes)?;

        if fixed_header.remaining_len == 2 {
            return Ok(PubComp { packet_id: pkid });
        }

        if fixed_header.remaining_len < 4 {
            return Ok(PubComp { packet_id: pkid });
        }

        let puback = PubComp { packet_id: pkid };

        Ok(puback)
    }

    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        let len = self.len();
        buffer.put_u8(0x70);
        let count = write_remaining_length(buffer, len);
        buffer.put_u16(self.packet_id);
        1 + count + len
    }
}
