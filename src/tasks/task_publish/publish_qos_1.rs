use crate::tasks::task_hub::HubMsg;
use crate::tasks::task_publish::PublishMsg;
use crate::tasks::Senders;
use crate::v3_1_1::Publish;
use crate::QoS;
use bytes::{Bytes, BytesMut};
use log::debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use tokio::time::{sleep, sleep_until, timeout, Instant};
use tokio::{select, spawn};

/// consider the order in which pushlish   are repeated
pub struct TaskPublishQos1 {
    tx: Senders,
    topic: Arc<String>,
    payload: Bytes,
    qos: QoS,
    retain: bool,
    pkid: u16,
}

impl TaskPublishQos1 {
    pub async fn init(
        tx: Senders,
        topic: Arc<String>,
        payload: Bytes,
        qos: QoS,
        retain: bool,
        pkid: u16,
    ) {
        spawn(async move {
            let mut publish = Self {
                tx,
                topic,
                payload,
                qos,
                retain,
                pkid,
            };
            publish.run().await;
        });
    }
    async fn run(&mut self) {
        debug!("start to Publish");
        let packet = Publish::new(
            self.topic.clone(),
            self.qos.clone(),
            self.payload.clone(),
            self.retain,
            Some(self.pkid),
        );
        let mut bytes = BytesMut::new();
        let mut rx_ack = self.tx.tx_publish.subscribe();

        packet.write(&mut bytes).unwrap();
        let data = bytes.freeze();
        let rx = self.tx.tx_network_default(data).await.unwrap();
        rx.await.unwrap();
        loop {
            match rx_ack.recv().await {
                Ok(ack) => match ack {
                    PublishMsg::Publish(_) => {}
                    PublishMsg::PubAck(ack) => {
                        if ack.pkid == self.pkid {
                            debug!("publish qos 1 success");
                            self.tx
                                .tx_hub
                                .send(HubMsg::RecoverId(self.pkid))
                                .await
                                .unwrap();
                            return;
                        }
                    }
                    PublishMsg::PubRec(_) => {}
                    PublishMsg::PubRel(_) => {}
                    PublishMsg::PubComp(_) => {}
                },
                Err(e) => {}
            }
        }
    }
    async fn timeout_rx(rx_ack: &mut Receiver<PublishMsg>, pkid: u16) -> anyhow::Result<()> {
        // let timer = &mut timeout(Duration::from_secs(3), sleep(Duration::from_secs(3)));
        let timer = A::init(Duration::from_secs(3));
        loop {
            select! {
                msg = rx_ack.recv() => match msg? {
                    PublishMsg::PubAck(ack) => {
                        if ack.pkid == pkid {
                            return Ok(());
                        }
                    }
                    _ => {}
                },
                _ = timer.sleep() => {

                }
            }
        }
    }
}

struct A {
    deadline: Instant,
}

impl A {
    fn init(after: Duration) -> Self {
        let deadline = Instant::now() + after;
        Self { deadline }
    }
    async fn sleep(&self) {
        sleep_until(self.deadline).await
    }
}
