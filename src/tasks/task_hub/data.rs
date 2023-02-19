use crate::datas::id::Id;
use crate::datas::payload::Payload;
use crate::tasks::task_client::data::{TracePublish, TraceSubscribe, TraceUnubscribe};
use crate::tasks::task_network::NetworkEvent;
use crate::v3_1_1::{ConnectReturnFailCode, Publish};
use crate::{ClientCommand, QoS};
use bytes::Bytes;
use std::default::Default;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

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
    BrokerRefuse(ConnectReturnFailCode),
}

impl Reason {
    pub fn to_msg(&self) -> String {
        match self {
            Reason::Init => "init".to_string(),
            Reason::NetworkErr(msg) => {
                format!("NetworkErr: {}", msg)
            }
            Reason::PingFail => "PingFail".to_string(),

            Reason::BrokerRefuse(code) => {
                format!("BrokerRefuse: {:?}", code)
            }
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

#[derive(Debug)]
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
#[derive(Debug)]
pub enum ToDisconnectReason {
    NetworkError(String),
    PingFail,
    ClientCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    // #[error("Network error")]
    // NetworkError(String),
    #[error("channel abnormal")]
    ChannelAbnormal,
    #[error("StateErr {0}")]
    StateErr(String),
    #[error("PacketIdErr {0}")]
    PacketIdErr(String),
    #[error("ViolenceDisconnectAndDrop")]
    ViolenceDisconnectAndDrop,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HubToConnectError {
    #[error("channel abnormal")]
    ChannelAbnormal,
    #[error("ViolenceDisconnectAndDrop")]
    ViolenceDisconnectAndDrop,
}

impl From<HubToConnectError> for HubError {
    fn from(value: HubToConnectError) -> Self {
        match value {
            HubToConnectError::ChannelAbnormal => HubError::ChannelAbnormal,
            HubToConnectError::ViolenceDisconnectAndDrop => HubError::ViolenceDisconnectAndDrop,
        }
    }
}
impl<T> From<broadcast::error::SendError<T>> for HubError {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for HubError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<broadcast::error::SendError<T>> for HubToConnectError {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for HubToConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
