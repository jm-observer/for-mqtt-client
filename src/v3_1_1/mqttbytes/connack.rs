use super::*;
use bytes::{Buf, Bytes};

/// Acknowledgement to connect packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnAck {
    pub session_present: bool,
    pub code: ConnectReturnCode,
}

impl ConnAck {
    pub fn new(code: ConnectReturnCode, session_present: bool) -> ConnAck {
        ConnAck {
            session_present,
            code,
        }
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);

        let flags = read_u8(&mut bytes)?;
        let return_code = read_u8(&mut bytes)?;

        let session_present = (flags & 0x01) == 1;
        let code = connect_return(return_code)?;
        let connack = ConnAck {
            session_present,
            code,
        };

        Ok(connack)
    }
    //
    // pub fn write(&self, buffer: &mut BytesMut) -> usize {
    //     let len = self.len();
    //     buffer.put_u8(0x20);
    //
    //     let count = write_remaining_length(buffer, len);
    //     buffer.put_u8(self.session_present as u8);
    //     buffer.put_u8(self.code as u8);
    //
    //     1 + count + len
    // }
}

#[cfg(test)]
mod test {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn connack_parsing_works() -> anyhow::Result<()> {
        let mut stream = bytes::BytesMut::new();
        let packetstream = &[
            0b0010_0000,
            0x02, // packet type, flags and remaining len
            0x01,
            0x00, // variable header. connack flags, connect return code
            0xDE,
            0xAD,
            0xBE,
            0xEF, // extra packets in the stream
        ];

        stream.extend_from_slice(&packetstream[..]);
        let fixed_header = parse_fixed_header(stream.iter())?;
        let connack_bytes = stream.split_to(fixed_header.frame_length()).freeze();
        let connack = ConnAck::read(fixed_header, connack_bytes)?;

        assert_eq!(
            connack,
            ConnAck {
                session_present: true,
                code: ConnectReturnCode::Success,
            }
        );
        Ok(())
    }

    #[test]
    fn connack_encoding_works() -> anyhow::Result<()> {
        let mut connack = ConnAck {
            session_present: true,
            code: ConnectReturnCode::Success,
        };

        let mut buf = BytesMut::new();
        connack.write(&mut buf)?;
        assert_eq!(buf, vec![0b0010_0000, 0x02, 0x01, 0x00]);
        Ok(())
    }
}
