use crate::v3_1_1::{ConnAck, Connect};
use bytes::Bytes;
use std::default::Default;
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
#[derive(Debug)]
pub enum HubMsg {
    RequestId(tokio::sync::oneshot::Sender<u16>),
    RecoverId(u16),
    Error,
}

#[derive(Debug, Clone)]
pub enum State {
    Connected,
    UnConnected(Reason),
}
#[derive(Debug, Clone)]
pub enum Reason {
    Init,
    NetworkErr(String),
}
impl State {
    pub fn is_connected(&self) -> bool {
        match self {
            State::Connected => true,
            _ => false,
        }
    }
}
impl Default for State {
    fn default() -> Self {
        Self::UnConnected(Reason::default())
    }
}
impl Default for Reason {
    fn default() -> Self {
        Self::Init
    }
}
