use super::*;
use bytes::{BufMut, BytesMut};
use for_event_bus_derive::Event;

pub struct PingReq;

impl PingReq {
    pub fn new() -> Bytes {
        let mut bytes = BytesMut::new();
        PingReq.write(&mut bytes);
        bytes.freeze()
    }
    fn write(&self, payload: &mut BytesMut) {
        payload.put_slice(&[0xC0, 0x00]);
    }
}
#[derive(Clone, Debug, Event)]
pub struct PingResp;

// impl PingResp {
//     pub fn write(&self, payload: &mut BytesMut) {
//         payload.put_slice(&[0xD0, 0x00]);
//     }
// }
