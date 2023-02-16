use crate::tasks::Receipter;
use crate::v3_1_1::{ConnectReturnCode, FixedHeaderError, PacketParseError, PacketType};
use bytes::Bytes;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use tokio::io;
use tokio::sync::broadcast;

pub enum NetworkState {
    ToConnect,
    Connected,
    ToDisconnect,
    Disconnected,
}

impl NetworkState {
    pub fn is_disconnected(&self) -> bool {
        if let Self::Disconnected = self {
            true
        } else {
            false
        }
    }
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
    Connected,
    Disconnected,
    Disconnect(String),
}

impl NetworkEvent {
    pub fn is_connected(&self) -> bool {
        if let Self::Connected = self {
            true
        } else {
            false
        }
    }
}

pub enum Data {
    NetworkData(DataWaitingToBeSend),
    // Reconnect,
    Disconnect,
}

impl Data {
    pub fn is_network_data(&self) -> bool {
        if let Self::NetworkData(_) = self {
            true
        } else {
            false
        }
    }
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::NetworkData(_) => {
                write!(f, "NetworkData")
            }
            Data::Disconnect => {
                write!(f, "Disconnect")
            }
        }
    }
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

impl From<DataWaitingToBeSend> for Data {
    fn from(val: DataWaitingToBeSend) -> Self {
        Self::NetworkData(val)
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
pub enum Error {
    #[error("Network error")]
    NetworkError(String),
    #[error("parse packet error")]
    PacketError(#[from] PacketParseError),
    #[error("recv data fail")]
    RecvDataFail,
    #[error("channel abnormal")]
    ChannelAbnormal,
}
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
    BrokerRefuse(ConnectReturnCode),
}
impl From<io::Error> for ToConnectError {
    fn from(err: io::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}
impl<T> From<broadcast::error::SendError<T>> for Error {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}
