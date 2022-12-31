mod data;

use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_network::NetworkMsg;
use crate::tasks::Senders;
use crate::v3_1_1::{Connect, MqttOptions, PingReq, Subscribe};
use crate::QoS;
use chrono::{Local, Timelike};
pub use data::*;
use log::{debug, error};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::spawn;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

pub struct TaskConnect {
    tx: Senders,
    tx_to_hub: mpsc::Sender<Connected>,
    options: MqttOptions,
}

impl TaskConnect {
    pub fn init(options: MqttOptions, tx: Senders, tx_to_hub: mpsc::Sender<Connected>) {
        spawn(async move {
            let mut subscriber = Self {
                tx_to_hub,
                tx,
                options,
            };
        });
    }
    async fn _run(&mut self) {
        debug!("start to connect");
        let connect = Arc::new(Connect::new(self.options.client_id().clone()).unwrap());
        let mut rx_ack = self.tx.subscribe_connect();
        loop {
            let receipter = self.tx.tx_network_default(connect.clone()).await.unwrap();
            match receipter.await {
                Ok(_) => {
                    debug!("done");
                }
                Err(e) => {
                    error!("fail to receive receipt");
                    continue;
                }
            }
            // consider timeout
            let result = timeout(Duration::from_secs(3), rx_ack.recv()).await;
            if let Ok(Ok(_)) = result {
                debug!("conn ack recv success");
                if let Err(e) = self.tx_to_hub.send(Connected).await {
                    error!("");
                }
                break;
            }
        }
    }
}
