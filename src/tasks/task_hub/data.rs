use crate::datas::payload::Payload;
use crate::datas::trace_publish::TracePublish;
use crate::tasks::task_network::NetworkStaus;
use crate::v3_1_1::Publish;
use crate::QoS;
use bytes::Bytes;
use std::default::Default;
use std::sync::Arc;

#[derive(Debug)]
pub enum HubMsg {
    // RequestId(tokio::sync::oneshot::Sender<u16>),
    RecoverId(u16),
    PingSuccess,
    PingFail,
    KeepAlive(KeepAliveTime),
    Subscribe {
        topic: String,
        qos: QoS,
    },
    Publish(TracePublish),
    /// 接收从broker发送的publish包
    RxPublish(Publish),
    /// 接收的publish已经完成qos流程
    RecoverRxId(u16),
    Unsubscribe {
        topic: String,
    },
    Disconnect,
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

impl From<NetworkStaus> for State {
    fn from(status: NetworkStaus) -> Self {
        match status {
            NetworkStaus::Connected => Self::Connected,
            NetworkStaus::Disconnect(error) => Self::UnConnected(Reason::NetworkErr(error)),
        }
    }
}
