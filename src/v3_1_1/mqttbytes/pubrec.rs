use super::*;
use crate::protocol::packet::{read_u16, write_remaining_length};
use crate::protocol::FixedHeader;
use anyhow::Result;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Acknowledgement to QoS2 publish
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubRec {
    pub packet_id: u16,
}

impl PubRec {
    pub fn new(packet_id: u16) -> Self {
        Self { packet_id }
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
            return Ok(PubRec { packet_id: pkid });
        }

        if fixed_header.remaining_len < 4 {
            return Ok(PubRec { packet_id: pkid });
        }

        let puback = PubRec { packet_id: pkid };

        Ok(puback)
    }

    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        let len = self.len();
        buffer.put_u8(0x50);
        let count = write_remaining_length(buffer, len);
        buffer.put_u16(self.packet_id);
        1 + count + len
    }
}
