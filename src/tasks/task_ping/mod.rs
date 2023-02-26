use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::CommonErr;
use crate::tasks::Senders;
use crate::v3_1_1::PingReq;
use log::debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::time::timeout;

/// consider the order in which pushlish   are repeated
pub struct TaskPing {
    tx: Senders,
}

impl TaskPing {
    pub fn init(tx: Senders) {
        spawn(async move {
            let mut ping = Self { tx };
            if let Err(e) = ping.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
    }
    async fn run(&mut self) -> Result<(), CommonErr> {
        let data = Arc::new(PingReq::new());
        let mut rx_ack = self.tx.subscribe_ping();
        let mut timeout_time = 3;
        while timeout_time > 0 {
            let result = timeout(
                Duration::from_secs(3),
                self.tx.tx_network_default(data.clone()),
            )
            .await;
            if let Ok(Ok(_)) = result {
                break;
            } else {
                timeout_time -= 1;
                if timeout_time <= 0 {
                    self.tx.tx_hub_msg.send(HubMsg::PingFail).await?
                }
            }
        }
        while timeout_time > 0 {
            debug!("wait for ping resp");
            let result = timeout(Duration::from_secs(3), rx_ack.recv()).await;
            if let Ok(Ok(_)) = result {
                debug!("ping resp recv success");
                self.tx.tx_hub_msg.send(HubMsg::PingSuccess).await?;
                return Ok(());
            } else {
                timeout_time -= 1;
            }
        }
        self.tx.tx_hub_msg.send(HubMsg::PingFail).await?;
        Ok(())
    }
}
