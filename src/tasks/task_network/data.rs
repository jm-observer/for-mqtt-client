use crate::tasks::Receipter;
use bytes::{Bytes, BytesMut};
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::Arc;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol::packet::ConnectReturnFailCode;
use crate::protocol::packet::Disconnect;
use crate::protocol::{NetworkProtocol, PacketParseError, PacketType};
use crate::tls::rustls::init_rustls;
use crate::tls::TlsConfig;
use anyhow::Result;
use tokio::sync::{broadcast, mpsc};
use tokio_rustls::client::TlsStream;

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
    BrokerDisconnect(Disconnect),
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
    #[error("rustls connect err")]
    RustlsConnectError(String),
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
    fn from(_err: ChannelAbnormal) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<ChannelAbnormal> for ToConnectError {
    fn from(_err: ChannelAbnormal) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for ToConnectError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}

pub enum Stream {
    Tcp(TcpStream),
    Rustls(TlsStream<TcpStream>),
}

impl Stream {
    pub async fn read_buf(&mut self, buf: &mut BytesMut) -> std::io::Result<usize> {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.read_buf(buf).await,
            Stream::Rustls(tls_stream) => tls_stream.read_buf(buf).await,
        }
    }

    pub async fn write_all(&mut self, datas: &[u8]) -> std::io::Result<()> {
        match self {
            Stream::Tcp(tcp_stream) => tcp_stream.write_all(datas).await,
            Stream::Rustls(tls_stream) => tls_stream.write_all(datas).await,
        }
    }

    pub async fn init(
        protocol: NetworkProtocol,
        addr: &String,
        port: u16,
    ) -> Result<Self, ToConnectError> {
        Ok(match protocol {
            NetworkProtocol::Tcp => {
                let stream = TcpStream::connect((addr.as_str(), port)).await?;
                stream.into()
            }
            NetworkProtocol::Tls(config) => Self::init_rustls(config, addr, port)
                .await
                .map_err(|x| ToConnectError::RustlsConnectError(x.to_string()))?,
        })
    }
    async fn init_rustls(config: TlsConfig, addr: &String, port: u16) -> Result<Self> {
        Ok({
            let connector = init_rustls(config)?;
            let stream = TcpStream::connect((addr.as_str(), port)).await?;
            let server_name = rustls::ServerName::try_from(addr.as_str())?;
            connector.connect(server_name, stream).await?.into()
        })
    }
}

impl From<TcpStream> for Stream {
    fn from(value: TcpStream) -> Self {
        Self::Tcp(value)
    }
}

impl From<TlsStream<TcpStream>> for Stream {
    fn from(value: TlsStream<TcpStream>) -> Self {
        Self::Rustls(value)
    }
}
