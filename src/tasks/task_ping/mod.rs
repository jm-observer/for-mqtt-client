use crate::protocol::packet::{PingReq, PingResp};
use crate::tasks::task_hub::HubMsg;
use crate::tasks::utils::CommonErr;
use crate::tasks::{HubError, Senders};
use for_event_bus::worker::IdentityOfSimple;
use for_event_bus::CopyOfBus;
use log::debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::time::timeout;

/// consider the order in which pushlish   are repeated
pub struct TaskPing {
    tx: Senders,
    rx: IdentityOfSimple<PingResp>,
}

impl TaskPing {
    pub async fn init(bus: CopyOfBus) -> Result<(), HubError> {
        let rx = bus.simple_login().await?;
        let tx = rx.tx();

        spawn(async move {
            let mut ping = Self {
                tx: Senders::init(tx),
                rx,
            };
            if let Err(e) = ping.run().await {
                match e {
                    CommonErr::ChannelAbnormal => {}
                }
            }
        });
        Ok(())
    }
    async fn run(&mut self) -> Result<(), CommonErr> {
        let data = Arc::new(PingReq::new());
        // self.rx.subscribe::<PingResp>()?;
        // let mut rx_ack = self.tx.subscribe_ping();
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
                    self.rx.dispatch_event(HubMsg::PingFail)?
                }
            }
        }
        while timeout_time > 0 {
            debug!("wait for ping resp");
            let result = timeout(Duration::from_secs(3), self.rx.recv()).await;
            if let Ok(Ok(_)) = result {
                debug!("ping resp recv success");
                self.rx.dispatch_event(HubMsg::PingSuccess)?;
                return Ok(());
            } else {
                timeout_time -= 1;
            }
        }
        self.rx.dispatch_event(HubMsg::PingFail)?;
        Ok(())
    }
}
