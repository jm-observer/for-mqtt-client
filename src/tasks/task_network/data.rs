use crate::tasks::Receipter;
use crate::v3_1_1::{
    ConnectReturnCode, ConnectReturnFailCode, FixedHeaderError, PacketParseError, PacketType,
};
use bytes::Bytes;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use tokio::io;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug)]
pub enum NetworkState {
    ToConnect,
    Connected,
    // 用error来替代后续的状态
    ToDisconnect,
    Disconnected,
}

impl NetworkState {
    pub fn is_to_disconnected(&self) -> bool {
        if let Self::ToDisconnect = self {
            true
        } else {
            false
        }
    }
    pub fn is_to_connect(&self) -> bool {
        if let Self::ToConnect = self {
            true
        } else {
            false
        }
    }
    pub fn is_connected(&self) -> bool {
        if let Self::Connected = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
/// broadcast network event
pub enum NetworkEvent {
    /// bool: session_present
    Connected(bool),
    ConnectFail(ToConnectError),
    /// 中间突然断开，network task发送后即drop
    ConnectedErr(String),
}

impl NetworkEvent {
    pub fn is_connected(&self) -> bool {
        if let Self::Connected(_) = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub enum HubNetworkCommand {
    Disconnect,
}

#[derive(Debug)]
pub struct DataWaitingToBeSend {
    pub(crate) data: Arc<Bytes>,
    pub(crate) receipter: Option<Receipter>,
}

impl DataWaitingToBeSend {
    pub fn init(data: Arc<Bytes>, receipter: Option<Receipter>) -> Self {
        Self { data, receipter }
    }
    pub fn done(self) {
        if let Some(receipter) = self.receipter {
            receipter.done();
        }
    }
}

impl Deref for DataWaitingToBeSend {
    type Target = Arc<Bytes>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// Error during serialization and deserialization
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkTasksError {
    #[error("Network error")]
    NetworkError(String),
    #[error("channel abnormal")]
    ChannelAbnormal,
    #[error("Command disconnect")]
    HubCommandToDisconnect,
    #[error("Connect fail: {0}")]
    ConnectFail(ToConnectError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelAbnormal;

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum ToConnectResult {
//     Success(TcpStream),
//     DisconnectByHub,
// }
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToConnectError {
    #[error("Expected ConnAck, received: {0:?}")]
    NotConnAck(PacketType),
    // #[error("Unexpected ConnAck")]
    // UnexpectedConnAck,
    #[error("Network error")]
    NetworkError(String),
    #[error("parse packet error")]
    PacketError(#[from] PacketParseError),
    #[error("broker refuse to connect")]
    BrokerRefuse(ConnectReturnFailCode),
    #[error("channel abnormal")]
    ChannelAbnormal,
}
impl From<io::Error> for ToConnectError {
    fn from(err: io::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}
impl<T> From<broadcast::error::SendError<T>> for NetworkTasksError {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for NetworkTasksError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<io::Error> for NetworkTasksError {
    fn from(err: io::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}
impl From<ToConnectError> for NetworkTasksError {
    fn from(err: ToConnectError) -> Self {
        Self::ConnectFail(err)
    }
}

impl From<ChannelAbnormal> for NetworkTasksError {
    fn from(err: ChannelAbnormal) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<ChannelAbnormal> for ToConnectError {
    fn from(err: ChannelAbnormal) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for ToConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
