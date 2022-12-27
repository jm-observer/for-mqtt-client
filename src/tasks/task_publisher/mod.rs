mod data;

use crate::tasks::Senders;
use crate::QoS;
use bytes::Bytes;
pub use data::*;
use std::sync::Arc;
use tokio::spawn;

/// consider the order in which pushlish   are repeated
pub struct TaskPublishMg {
    tx: Senders,
    topic: Arc<String>,
    qos: QoS,
    payload: Arc<Bytes>,
}

impl TaskPublishMg {
    pub async fn run(self) {
        spawn(async move {});
    }
}
