use crate::protocol::packet::{write_mqtt_bytes, write_remaining_length};
use bytes::{BufMut, Bytes, BytesMut};

/// Subscription packet
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subscribe {
    V4 {
        packet_id: u16,
        payload: Bytes,
    },
    V5 {
        packet_id: u16,
        properties: Bytes,
        filters: Bytes,
    },
}

impl Subscribe {
    pub fn set_packet_id(&mut self, new_packet_id: u16) {
        match self {
            Subscribe::V4 { packet_id, .. } => *packet_id = new_packet_id,
            Subscribe::V5 { packet_id, .. } => *packet_id = new_packet_id,
        }
    }
    pub fn packet_id(&self) -> u16 {
        match self {
            Subscribe::V4 { packet_id, .. } => *packet_id,
            Subscribe::V5 { packet_id, .. } => *packet_id,
        }
    }
    pub fn write(&self, buffer: &mut BytesMut) -> usize {
        match self {
            Self::V4 {
                packet_id,
                payload: filters,
            } => {
                // write packet type
                buffer.put_u8(0x82);
                // write remaining length
                let remaining_len = filters.len() + 2;
                let remaining_len_bytes = write_remaining_length(buffer, remaining_len);

                // write packet id
                buffer.put_u16(*packet_id);

                write_mqtt_bytes(buffer, filters.as_ref());

                1 + remaining_len_bytes + remaining_len
            }
            Self::V5 { .. } => {
                todo!()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub path: String,
    pub options: SubscribeOptions,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscribeOptions(u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainForwardRule {
    OnEverySubscribe,
    OnNewSubscribe,
    Never,
}

impl RetainForwardRule {
    pub fn merge_to_u8(&self, val: &mut u8) {
        match self {
            RetainForwardRule::OnEverySubscribe => {}
            RetainForwardRule::OnNewSubscribe => *val |= 1 << 4,
            RetainForwardRule::Never => *val |= 2 << 4,
        }
    }
}

impl Default for RetainForwardRule {
    fn default() -> Self {
        Self::OnEverySubscribe
    }
}
//
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct SubscribeProperties {

// }

pub struct VariableHeaderV4 {
    packet_id: u16,
}
pub struct VariableHeaderV5 {
    packet_id: u16,
    id: Option<usize>,
    user_properties: Bytes,
}
