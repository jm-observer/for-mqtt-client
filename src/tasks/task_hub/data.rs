use for_event_bus::BusError;
use for_event_bus_derive::Event;
use log::warn;
use std::{
    default::Default,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::protocol::packet::Publish;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug, Event)]
pub enum HubMsg {
    // RequestId(tokio::sync::oneshot::Sender<u16>),
    /// recover packet_id that used to publish/subscribe/unsubscribe
    /// by client.
    RecoverId(u16),
    PingSuccess,
    PingFail,
    KeepAlive(KeepAliveTime),
    /// 接收从broker发送的publish包
    RxPublish(Publish),
    /// 接收的publish已经完成整个qos流程. recover packet_id that used
    /// to publish by broker.
    AffirmRxId(u16),
    /// 确认qos=2的publish包
    AffirmRxPublish(u16),
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
    PingFail,
    NetworkErr(String),
    ClientCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    #[error("channel abnormal")]
    ChannelAbnormal,
    // #[error("StateErr {0}")]
    // StateErr(String),
    #[error("PacketIdErr {0}")]
    PacketIdErr(String),
    #[error("ViolenceDisconnectAndDrop")]
    ViolenceDisconnectAndDrop,
    #[error("Other {0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HubToConnectError {
    #[error("channel abnormal")]
    ChannelAbnormal,
    #[error("ViolenceDisconnectAndDrop")]
    ViolenceDisconnectAndDrop,
    #[error("Other {0}")]
    Other(String),
}

impl From<HubToConnectError> for HubError {
    fn from(value: HubToConnectError) -> Self {
        match value {
            HubToConnectError::ChannelAbnormal => {
                HubError::ChannelAbnormal
            },
            HubToConnectError::ViolenceDisconnectAndDrop => {
                HubError::ViolenceDisconnectAndDrop
            },
            HubToConnectError::Other(err) => HubError::Other(err),
        }
    }
}
impl From<BusError> for HubError {
    fn from(value: BusError) -> Self {
        match value {
            BusError::ChannelErr => Self::ChannelAbnormal,
            BusError::DowncastErr => {
                warn!("downcast err");
                Self::ChannelAbnormal
            },
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

impl From<BusError> for HubToConnectError {
    fn from(err: BusError) -> Self {
        match err {
            BusError::ChannelErr => Self::ChannelAbnormal,
            BusError::DowncastErr => {
                warn!("downcast err");
                Self::ChannelAbnormal
            },
        }
    }
}
