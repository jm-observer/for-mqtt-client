#![allow(dead_code, unused_mut, unused_imports, unused_variables)]

use anyhow::Result;
use for_mqtt_client::protocol::MqttOptions;
use for_mqtt_client::QoS;
use for_mqtt_client::{MqttEvent, ProtocolV4};
use log::LevelFilter::{Debug, Info};
use log::{debug, error, info, warn};
use std::io::Read;
use std::time::Duration;
use tokio::spawn;
use tokio::time::sleep;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    custom_utils::logger::custom_build(Debug)
        // .module("for_mqtt_client::tasks::task_network", Debug)
        // .module("for_mqtt_client::tasks::task_publish", Debug)
        // .module("for_mqtt_client::tasks::task_hub", Debug)
        .build_default()
        .log_to_stdout()
        .start();
    let mut options = MqttOptions::new_v4("abc111".to_string(), "broker.emqx.io".to_string(), 1883);
    options.set_keep_alive(30);
    let _client = options.connect::<ProtocolV4>().await;
    let mut event_rx = _client.init_receiver();
    spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                MqttEvent::ConnectSuccess(session_present) => {
                    info!("\nConnectSuccess {}\n", session_present);
                }
                MqttEvent::ConnectFail(reason) => {
                    info!("\nConnectFail：{} \n", reason);
                }
                MqttEvent::Publish(packet) => {
                    info!("\nRx Publish：{:x?} \n", packet.payload.as_ref());
                }
                MqttEvent::PublishSuccess(id) => {
                    info!("\nPublish Success：{} \n", id);
                }
                MqttEvent::SubscribeAck(ack) => {
                    info!("\nSubscribeAck：{:?} \n", ack);
                }
                MqttEvent::UnsubscribeAck(ack) => info!("\nUnsubscribeAck：{:?} \n", ack),
                event => {
                    info!("\nMqttEvent：{:?} \n", event);
                }
            }
        }
        warn!("**************");
    });

    sleep(Duration::from_secs(3)).await;
    _client.disconnect().await?;
    sleep(Duration::from_secs(5)).await;
    Ok(())
}
