use crate::datas::id::Id;
use crate::datas::payload::Payload;
use crate::tasks::task_client::data::{TracePublish, TraceSubscribe, TraceUnubscribe};
use crate::tasks::task_network::NetworkEvent;
use crate::v3_1_1::Publish;
use crate::{ClientCommand, QoS};
use bytes::Bytes;
use std::default::Default;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub enum HubMsg {
    // RequestId(tokio::sync::oneshot::Sender<u16>),
    RecoverId(u16),
    PingSuccess,
    PingFail,
    KeepAlive(KeepAliveTime),
    /// 接收从broker发送的publish包
    RxPublish(Publish),
    /// 接收的publish已经完成整个qos流程
    AffirmRxId(u16),
    /// 确认qos=2的publish包
    AffirmRxPublish(u16),
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

impl Reason {
    pub fn to_msg(&self) -> String {
        match self {
            Reason::Init => "init".to_string(),
            Reason::NetworkErr(msg) => {
                format!("NetworkErr: {}", msg)
            }
            Reason::PingFail => "PingFail".to_string(),
        }
    }
}

/// 仅限
static KEEP_ALIVE_ID: AtomicU32 = AtomicU32::new(0);

impl Default for KeepAliveTime {
    fn default() -> Self {
        KEEP_ALIVE_ID.fetch_add(1, Ordering::Relaxed);
        Self(KEEP_ALIVE_ID.load(Ordering::Relaxed))
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeepAliveTime(u32);

impl KeepAliveTime {
    pub fn latest(&self) -> bool {
        self.0 == KEEP_ALIVE_ID.load(Ordering::Relaxed)
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

impl From<NetworkEvent> for State {
    fn from(status: NetworkEvent) -> Self {
        match status {
            NetworkEvent::Connected => Self::Connected,
            NetworkEvent::Disconnect(error) => Self::UnConnected(Reason::NetworkErr(error)),
            NetworkEvent::Disconnected => {
                todo!()
            }
        }
    }
}

pub enum HubState {
    ToConnect,
    Connected,
    ToDisconnect(ToDisconnectReason),
    Disconnected,
}

impl HubState {
    pub fn is_to_connect(&self) -> bool {
        match self {
            HubState::ToConnect => true,
            _ => false,
        }
    }
    pub fn is_connected(&self) -> bool {
        match self {
            HubState::Connected => true,
            _ => false,
        }
    }
    pub fn is_to_disconnect(&self) -> bool {
        match self {
            HubState::ToDisconnect(_) => true,
            _ => false,
        }
    }
    pub fn is_disconnected(&self) -> bool {
        match self {
            HubState::Disconnected => true,
            _ => false,
        }
    }
}

impl Default for HubState {
    fn default() -> Self {
        Self::ToConnect
    }
}

pub enum ToDisconnectReason {
    NetworkError(String),
    PingFail,
    ClientCommand,
}
