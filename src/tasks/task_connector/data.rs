use crate::v3_1_1::ConnAck;

pub enum ConnectMsg {
    ConnAck(ConnAck),
    PingResp,
    Error,
}

impl From<ConnAck> for ConnectMsg {
    fn from(val: ConnAck) -> Self {
        Self::ConnAck(val)
    }
}
