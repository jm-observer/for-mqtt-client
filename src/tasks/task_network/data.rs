use crate::tasks::Receipter;
use bytes::Bytes;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use std::sync::Arc;

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

pub enum Data {
    NetworkData(DataWaitingToBeSend),
    Reconnect,
    Disconnect,
}

impl Debug for Data {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Data::NetworkData(_) => {
                write!(f, "NetworkData")
            }
            Data::Reconnect => {
                write!(f, "Reconnect")
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
