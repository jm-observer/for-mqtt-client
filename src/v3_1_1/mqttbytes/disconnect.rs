use super::*;
use bytes::{BufMut, BytesMut};

pub struct Disconnect;

pub static DISCONNECT_DATA: [u8; 2] = [0xE0, 0x00];

impl Disconnect {
    // pub fn write(&self, payload: &mut BytesMut) -> Result<usize, Error> {
    //     payload.put_slice(&[0xE0, 0x00]);
    //     Ok(2)
    // }
    pub fn data() -> &'static [u8] {
        &DISCONNECT_DATA
    }
}
