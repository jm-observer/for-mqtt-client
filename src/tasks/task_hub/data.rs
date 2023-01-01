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
    PingSuccess,
    PingFail,
    KeepAlive(KeepAliveTime),
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
    PingFail,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeepAliveTime(u64);

impl KeepAliveTime {
    pub fn init() -> Self {
        Self(0)
    }
    pub fn update(&mut self) -> Self {
        if let Some(time) = self.0.checked_add(1) {
            self.0 = time;
        } else {
            self.0 = 0;
        }
        let old = self.clone();
        old
    }
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
