use crate::tasks::Senders;
use crate::traits::packet_dup::PacketDup;
use crate::traits::packet_rel::PacketRel;
use bytes::Bytes;
use for_event_bus::worker::IdentityOfSimple;
use for_event_bus::BusError;
use log::debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};

pub async fn complete_to_tx_packet<Ack: PacketRel, T: PacketDup>(
    rx_ack: &mut IdentityOfSimple<Ack>,
    packet_id: u16,
    duration: u64,
    tx: &Senders,
    packet: &mut T,
) -> anyhow::Result<Arc<Ack>, CommonErr> {
    let data = packet.data();
    let mut dup_data = Option::<Arc<Bytes>>::None;
    tx.tx_network_default(data).await?;
    loop {
        if let Ok(packet) =
            tokio::time::timeout(Duration::from_secs(duration), timeout_rx(rx_ack, packet_id)).await
        {
            return Ok(packet?);
        } else {
            let data = if let Some(data) = &dup_data {
                data.clone()
            } else {
                let data = packet.dup_data();
                dup_data = Some(data.clone());
                data
            };
            tx.tx_network_default(data.clone()).await?;
        }
    }
}

async fn timeout_rx<T: PacketRel>(
    rx_ack: &mut IdentityOfSimple<T>,
    packet_id: u16,
) -> anyhow::Result<Arc<T>, CommonErr> {
    loop {
        let msg = rx_ack.recv().await?;
        if msg.is_rel(packet_id) {
            debug!("rx success: {:?}", msg);
            return Ok(msg);
        }
    }
}

#[derive(Debug)]
pub enum CommonErr {
    ChannelAbnormal,
}

impl<T> From<broadcast::error::SendError<T>> for CommonErr {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl<T> From<mpsc::error::SendError<T>> for CommonErr {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<oneshot::error::RecvError> for CommonErr {
    fn from(_: oneshot::error::RecvError) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<broadcast::error::RecvError> for CommonErr {
    fn from(_: broadcast::error::RecvError) -> Self {
        Self::ChannelAbnormal
    }
}
impl From<BusError> for CommonErr {
    fn from(err: BusError) -> Self {
        match err {
            BusError::ChannelErr => Self::ChannelAbnormal,
        }
    }
}
// impl From<Elapsed> for CommonErr {
//     fn from(_: Elapsed) -> Self {
//         Self::Elapsed
//     }
// }
