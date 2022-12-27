use crate::v3_1_1::{ConnAck, Connect};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;

#[derive(Debug)]
pub enum HubMsg {
    RequestId(tokio::sync::oneshot::Sender<u16>),
    RecoverId(u16),
    ConnAck(ConnAck),
    PingResp,
    Error,
    NetworkConnectSuccess,
    NetworkConnectFail(String),
}
