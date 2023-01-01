use crate::tasks::Receipter;
use bytes::Bytes;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug, Clone)]
/// broadcast network status
pub enum NetworkStaus {
    Connected,
    Disconnect(String),
}

#[derive(Debug)]
pub enum Data {
    NetworkData(DataWaitingToBeSend),
    Reconnect,
}

#[derive(Debug)]
pub struct DataWaitingToBeSend {
    pub(crate) data: Arc<Bytes>,
    pub(crate) receipter: Receipter,
}

impl DataWaitingToBeSend {
    pub fn init(data: Arc<Bytes>, receipter: Receipter) -> Self {
        Self { data, receipter }
    }
    pub fn done(self) {
        self.receipter.done();
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
