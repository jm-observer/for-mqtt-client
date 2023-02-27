use super::*;
use crate::protocol::packet::{
    read_mqtt_string, read_u16, write_mqtt_string, write_remaining_length,
};
use crate::protocol::FixedHeader;
use anyhow::Result;
use bytes::{Buf, Bytes};
use std::sync::Arc;

/// Unsubscribe packet
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsubscribe {
    pub packet_id: u16,
    pub topics: Vec<Arc<String>>,
}

impl Unsubscribe {
    pub fn new(topics: Vec<Arc<String>>, packet_id: u16) -> Self {
        Unsubscribe { packet_id, topics }
    }

    pub fn read(fixed_header: FixedHeader, mut bytes: Bytes) -> Result<Self, PacketParseError> {
        let variable_header_index = fixed_header.fixed_header_len;
        bytes.advance(variable_header_index);

        let pkid = read_u16(&mut bytes)?;
        let mut payload_bytes = fixed_header.remaining_len - 2;
        let mut topics = Vec::with_capacity(1);

        while payload_bytes > 0 {
            let topic_filter = read_mqtt_string(&mut bytes)?;
            payload_bytes -= topic_filter.len() + 2;
            topics.push(topic_filter.into());
        }

        let unsubscribe = Unsubscribe {
            packet_id: pkid,
            topics,
        };
        Ok(unsubscribe)
    }

    pub fn write(&self, payload: &mut BytesMut) -> usize {
        let remaining_len = 2 + self.topics.iter().fold(0, |s, topic| s + topic.len() + 2);

        payload.put_u8(0xA2);
        let remaining_len_bytes = write_remaining_length(payload, remaining_len);
        payload.put_u16(self.packet_id);

        for topic in self.topics.iter() {
            write_mqtt_string(payload, topic.as_str());
        }
        1 + remaining_len_bytes + remaining_len
    }
}
