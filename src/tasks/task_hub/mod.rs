mod data;

use std::time::Duration;
use tokio::sync::broadcast::{channel, Sender};
use crate::tasks::task_network::{NetworkData, TaskNetwork};
use crate::utils::Endpoint;
use crate::v3_1_1::{Client, Connect, MqttOptions};
use anyhow::Result;
use tokio::sync::mpsc;
use crate::tasks::{InnerCommand, MqttEvent, Senders, UserMsg};
use crate::tasks::task_connector::TaskConnector;

pub struct TaskHub {
    options: MqttOptions,
    user_endpoint: Endpoint<UserMsg, MqttEvent>,
    mqtt_endpoint: Endpoint<MqttEvent, UserMsg>,
    senders: Senders,
}
impl TaskHub {
    pub async fn init(options: MqttOptions) -> Client {
        let (user_endpoint, mqtt_endpoint) = Endpoint::channel(1024);

        let (tx_broadcast, _) = channel(1024);
        let (tx_network_writer, rx_network_writer) = mpsc::channel(1024);
        let (tx_connector, rx_connector) = mpsc::channel(1024);
        let (tx_publisher, rx_publisher) = mpsc::channel(1024);
        let (tx_subscriber, rx_subscriber) = mpsc::channel(1024);

        let senders = Senders::init(tx_network_writer, tx_connector, tx_publisher, tx_subscriber, tx_broadcast);

        let (addr, port) = options.broker_address();
        let network_task =
            TaskNetwork::init(addr.parse().unwrap(), port, senders.clone(), rx_network_writer);
        let task_connetor = TaskConnector::init(senders.clone(), options.clone(), rx_connector);

        let hub = Self {
            options, user_endpoint, mqtt_endpoint,
            senders
        };
        task_connetor.run().await;
        network_task.run().await;

        let client = Client::init(hub.user_endpoint.clone());
        tokio::spawn(async move {

        });
        client
    }
    async fn run(self) -> Result<()> {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(())
    }

    async fn deal_inner_msg(&mut self, msg: InnerCommand) -> Result<()> {
        match msg {
            InnerCommand::NetworkConnectSuccess => {

            }
            InnerCommand::NetworkConnectFail(_) => {}
        }

        Ok(())
    }
}