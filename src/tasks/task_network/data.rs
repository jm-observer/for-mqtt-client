use crate::tasks::Receipter;
use bytes::{Bytes, BytesMut};
use std::{fmt::Debug, ops::Deref, sync::Arc};
use tokio::{
    io,
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::protocol::{
    packet::{ConnectReturnFailCode, Disconnect},
    NetworkProtocol, PacketParseError, PacketType,
};
#[cfg(feature = "tls")]
use crate::tls::{rustls::init_rustls, TlsConfig};
use anyhow::Result;
use for_event_bus::{BusError, Event, Merge};
use log::warn;
use tokio::sync::{broadcast, mpsc};
#[cfg(feature = "tls")]
use tokio_rustls::client::TlsStream;

#[derive(Debug, Copy, Clone)]
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

    pub fn is_connected(&self) -> bool {
        if let Self::Connected = self {
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Event)]
/// broadcast network event
pub enum NetworkEvent {
    /// bool: session_present
    Connected(bool),
    ConnectFail(ToConnectError),
    /// 中间突然断开，network task发送后即drop
    ConnectedErr(String),
    /// broker send disconnect packet
    BrokerDisconnect(Disconnect),
}
//
// #[derive(Debug, Clone)]
// pub enum TaskEvent {
//     HubNetworkCommand(Arc<HubNetworkCommand>),
//     DataWaitingToBeSend(Arc<DataWaitingToBeSend>),
// }

#[derive(Debug, Event, Clone)]
pub enum HubNetworkCommand {
    Disconnect,
}

#[derive(Merge, Event)]
pub enum NetworkData {
    Command(HubNetworkCommand),
    Data(DataWaitingToBeSend),
}

#[derive(Debug, Event, Clone)]
pub struct DataWaitingToBeSend {
    pub(crate) data: Arc<Bytes>,
    pub(crate) receipter: Option<Arc<Receipter>>,
}

impl DataWaitingToBeSend {
    pub fn init(
        data: Arc<Bytes>,
        receipter: Option<Arc<Receipter>>,
    ) -> Self {
        Self { data, receipter }
    }

    pub fn done(&self) {
        if let Some(receipter) = &self.receipter {
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
    ConnectFail(ToConnectError), /* #[error("BusErr")]
                                  * BusErr, */
}

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
    #[error("rustls connect err")]
    RustlsConnectError(String),
}
impl From<io::Error> for ToConnectError {
    fn from(err: io::Error) -> Self {
        Self::NetworkError(err.to_string())
    }
}

impl From<BusError> for ToConnectError {
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

impl From<BusError> for NetworkTasksError {
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

// impl From<ChannelAbnormal> for NetworkTasksError {
//     fn from(_err: ChannelAbnormal) -> Self {
//         Self::ChannelAbnormal
//     }
// }
// impl From<ChannelAbnormal> for ToConnectError {
//     fn from(_err: ChannelAbnormal) -> Self {
//         Self::ChannelAbnormal
//     }
// }
impl<T> From<mpsc::error::SendError<T>> for ToConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}

pub enum Stream {
    Tcp(TcpStream),
    #[cfg(feature = "tls")]
    Rustls(TlsStream<TcpStream>),
}

impl Stream {
    pub async fn read_buf(
        &mut self,
        buf: &mut BytesMut,
    ) -> std::io::Result<usize> {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.read_buf(buf).await,
            #[cfg(feature = "tls")]
            Stream::Rustls(tls_stream) => {
                tls_stream.read_buf(buf).await
            },
        }
    }

    pub async fn write_all(
        &mut self,
        datas: &[u8],
    ) -> std::io::Result<()> {
        match self {
            Stream::Tcp(tcp_stream) => {
                tcp_stream.write_all(datas).await
            },
            #[cfg(feature = "tls")]
            Stream::Rustls(tls_stream) => {
                tls_stream.write_all(datas).await
            },
        }
    }

    pub async fn init(
        protocol: NetworkProtocol,
        addr: &String,
        port: u16,
    ) -> Result<Self, ToConnectError> {
        Ok(match protocol {
            NetworkProtocol::Tcp => {
                let stream =
                    TcpStream::connect((addr.as_str(), port)).await?;
                stream.into()
            },
            #[cfg(feature = "tls")]
            NetworkProtocol::Tls(config) => Self::init_rustls(
                config, addr, port,
            )
            .await
            .map_err(|x| {
                ToConnectError::RustlsConnectError(x.to_string())
            })?,
        })
    }

    #[cfg(feature = "tls")]
    async fn init_rustls(
        config: TlsConfig,
        addr: &String,
        port: u16,
    ) -> Result<Self> {
        Ok({
            let connector = init_rustls(config)?;
            let stream =
                TcpStream::connect((addr.as_str(), port)).await?;
            let server_name =
                rustls::ServerName::try_from(addr.as_str())?;
            connector.connect(server_name, stream).await?.into()
        })
    }
}

impl From<TcpStream> for Stream {
    fn from(value: TcpStream) -> Self {
        Self::Tcp(value)
    }
}

#[cfg(feature = "tls")]
impl From<TlsStream<TcpStream>> for Stream {
    fn from(value: TlsStream<TcpStream>) -> Self {
        Self::Rustls(value)
    }
}
