use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_network::NetworkMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{PingReq, Subscribe};
use crate::QoS;
use chrono::{Local, Timelike};
use log::{debug, error};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::spawn;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// consider the order in which pushlish   are repeated
pub struct TaskPing {
    tx: Senders,
}

impl TaskPing {
    pub fn init(tx: Senders) {
        spawn(async move {
            let mut subscriber = Self { tx };
        });
    }
    async fn _run(&mut self) {
        debug!("start to ping");
        let data = Arc::new(PingReq::new());
        let mut rx_ack = self.tx.subscribe_ping();
        let mut timeout_time = 3;
        while timeout_time > 0 {
            let rx = self.tx.tx_network_default(data.clone()).await.unwrap();
            let result = timeout(Duration::from_secs(3), rx).await;
            if let Ok(Ok(_)) = result {
                debug!("ping success");
                break;
            } else {
                timeout_time -= 1;
                if timeout_time <= 0 {
                    if let Err(e) = self.tx.tx_hub.send(HubMsg::PingFail).await {
                        error!("");
                    }
                    return;
                }
            }
        }
        while timeout_time > 0 {
            let rx = self.tx.tx_network_default(data.clone()).await.unwrap();
            let result = timeout(Duration::from_secs(3), rx_ack.recv()).await;
            if let Ok(Ok(_)) = result {
                debug!("ping resp recv success");
                if let Err(e) = self.tx.tx_hub.send(HubMsg::PingSuccess).await {
                    error!("");
                }
                break;
            } else {
                timeout_time -= 1;
            }
        }
        if let Err(e) = self.tx.tx_hub.send(HubMsg::PingFail).await {
            error!("");
        }
    }
}
