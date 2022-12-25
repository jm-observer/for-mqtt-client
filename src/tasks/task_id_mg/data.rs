use std::sync::Arc;
use tokio::sync::RwLock;
use crate::tasks::*;

pub enum PacketIdMsg {
    RequestId(tokio::sync::oneshot::Sender<Id>),
    RecoverId(Id)
}
pub struct Id {
    id: u16,
    status: Arc<RwLock<Status>>,
}
#[derive(Debug, Clone)]
enum Status {
    Free,
    Busy
}


impl Default for Status {
    fn default() -> Self {
        Self::Free
    }
}