use crate::protocol::packet::publish::Publish;
use crate::protocol::packet::{PubAck, PubComp, PubRec, PubRel};

#[derive(Clone, Debug)]
pub enum PublishMsg {
    Publish(Publish),
    PubAck(PubAck),
    PubRec(PubRec),
    PubRel(PubRel),
    PubComp(PubComp),
}
impl From<PubComp> for PublishMsg {
    fn from(val: PubComp) -> Self {
        Self::PubComp(val)
    }
}
impl From<PubRel> for PublishMsg {
    fn from(val: PubRel) -> Self {
        Self::PubRel(val)
    }
}
impl From<PubRec> for PublishMsg {
    fn from(val: PubRec) -> Self {
        Self::PubRec(val)
    }
}
impl From<Publish> for PublishMsg {
    fn from(val: Publish) -> Self {
        Self::Publish(val)
    }
}
impl From<PubAck> for PublishMsg {
    fn from(val: PubAck) -> Self {
        Self::PubAck(val)
    }
}
