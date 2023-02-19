use crate::tasks::Senders;
use crate::traits::packet_dup::PacketDup;
use crate::traits::packet_rel::PacketRel;
use anyhow::bail;
use bytes::Bytes;
use log::{debug, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::Receiver;
use tokio::time::error::Elapsed;

pub async fn complete_to_tx_packet<Ack: PacketRel, T: PacketDup>(
    rx_ack: &mut Receiver<Ack>,
    pkid: u16,
    duration: u64,
    tx: &Senders,
    packet: &mut T,
) -> anyhow::Result<Ack> {
    let mut data = packet.data();
    let mut dup_data = Option::<Arc<Bytes>>::None;
    if tx.tx_network_default(data).await.is_err() {
        warn!("todo");
    }
    loop {
        let Ok(packet) = timeout_rx(rx_ack, pkid, duration).await else {
            let data = if let Some(data) = &dup_data {
                data.clone()
            } else {
                let data = packet.dup_data();
                dup_data = Some(data.clone());
                data
            };
            if tx.tx_network_default(data.clone()).await.is_err() {
                warn!("todo");
            }
            continue;
        };
        return Ok(packet);
    }
}

pub async fn timeout_rx<T: PacketRel>(
    rx_ack: &mut Receiver<T>,
    pkid: u16,
    duration: u64,
) -> anyhow::Result<T> {
    let Ok(result) = tokio::time::timeout(Duration::from_secs(duration), _timeout_rx(rx_ack, pkid)).await else {
        bail!("timeout");
    };
    result
}

async fn _timeout_rx<T: PacketRel>(rx_ack: &mut Receiver<T>, pkid: u16) -> anyhow::Result<T> {
    while let Ok(msg) = rx_ack.recv().await {
        if msg.is_rel(pkid) {
            debug!("rx success: {:?}", msg);
            return Ok(msg);
        }
    }
    bail!("todo");
}
