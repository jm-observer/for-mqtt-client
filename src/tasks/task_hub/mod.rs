mod data;

use crate::tasks::task_network::{NetworkData, TaskNetwork};
use crate::tasks::{MqttEvent, Senders, UserMsg};
use crate::utils::Endpoint;
use crate::v3_1_1::{Client, Connect, MqttOptions};
use anyhow::Result;
use log::{debug, error};
use std::time::Duration;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::mpsc;

pub use data::HubMsg;

pub struct TaskHub {
    options: MqttOptions,
    senders: Senders,
    rx: mpsc::Receiver<HubMsg>,
}
impl TaskHub {
    pub async fn init(options: MqttOptions) -> (Client, Receiver<MqttEvent>) {
        let (tx_user, _) = channel(1024);
        let (tx_network_writer, rx_network_writer) = mpsc::channel(1024);
        let (tx_publisher, rx_publisher) = mpsc::channel(1024);
        let (tx_subscriber, _) = channel(1024);
        let (tx_hub, rx_hub) = mpsc::channel(1024);

        let senders = Senders::init(
            tx_network_writer,
            tx_publisher,
            tx_subscriber,
            tx_user,
            tx_hub,
        );

        let (addr, port) = options.broker_address();
        let network_task = TaskNetwork::init(
            addr.parse().unwrap(),
            port,
            senders.clone(),
            rx_network_writer,
        );
        let hub = Self {
            options,
            senders: senders.clone(),
            rx: rx_hub,
        };
        let client = Client::init(senders);
        let event_rx = client.init_receiver();
        network_task.run().await;

        tokio::spawn(async move {
            hub.run().await.unwrap();
        });
        (client, event_rx)
    }
    async fn run(mut self) -> Result<()> {
        let (mut a, mut b) = ringbuf::SharedRb::new(65535).split();
        for i in 1..=u16::MAX {
            a.push(i).unwrap();
        }
        loop {
            match self.rx.recv().await {
                Some(req) => match req {
                    HubMsg::RequestId(req) => {
                        debug!("request id");
                        if let Some(id) = b.pop() {
                            req.send(id).unwrap();
                        } else {
                            todo!()
                        }
                    }
                    HubMsg::RecoverId(id) => {
                        a.push(id).unwrap();
                    }
                    HubMsg::ConnAck(ack) => {
                        if ack.code.is_success() {
                            debug!("connect success");
                            // self.senders.tx_mqtt_event(MqttEvent::ConnectSuccess);
                        } else {
                            todo!()
                            // self.senders
                            //     .tx_mqtt_event(MqttEvent::ConnectFail(format!("{:?}", ack.code)));
                        }
                    }
                    HubMsg::PingResp => {}
                    HubMsg::Error => {}
                    HubMsg::NetworkConnectSuccess => {
                        let connect = Connect::new(self.options.client_id().clone()).unwrap();
                        let receipter = self.senders.tx_network_default(connect).await.unwrap();
                        match receipter.await {
                            Ok(_) => {
                                debug!("done");
                            }
                            Err(e) => {
                                error!("fail to receive receipt")
                            }
                        }
                    }
                    HubMsg::NetworkConnectFail(_) => {}
                },
                None => {}
            }
        }
    }
}
