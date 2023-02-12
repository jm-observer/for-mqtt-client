use bytes::Bytes;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq)]
pub enum Payload {
    String(Arc<String>),
    Bytes(Arc<Bytes>),
}

impl Payload {
    pub fn len(&self) -> usize {
        match self {
            Payload::String(val) => val.as_bytes().len(),
            Payload::Bytes(val) => val.len(),
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Payload::String(val) => val.as_bytes(),
            Payload::Bytes(val) => val.as_ref(),
        }
    }
}

impl Debug for Payload {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Payload::String(val) => {
                write!(f, "Payload: {:?}", val)
            }
            Payload::Bytes(val) => {
                write!(f, "Payload: Bytes({})", val.len())
            }
        }
    }
}

// impl From<String> for Payload {
//     fn from(val: String) -> Self {
//         Self::String(val)
//     }
// }
// impl From<&str> for Payload {
//     fn from(val: &str) -> Self {
//         Self::String(val.into())
//     }
// }
// impl From<Bytes> for Payload {
//     fn from(val: Bytes) -> Self {
//         Self::Bytes(val)
//     }
// }
