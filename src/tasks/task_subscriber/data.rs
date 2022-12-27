use crate::v3_1_1::{SubAck, UnsubAck};

#[derive(Debug, Clone)]
pub enum SubscribeMsg {
    SubAck(SubAck),
    UnsubAck(UnsubAck),
}
impl From<UnsubAck> for SubscribeMsg {
    fn from(val: UnsubAck) -> Self {
        Self::UnsubAck(val)
    }
}
impl From<SubAck> for SubscribeMsg {
    fn from(val: SubAck) -> Self {
        Self::SubAck(val)
    }
}
