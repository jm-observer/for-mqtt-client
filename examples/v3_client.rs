#![allow(dead_code, unused_mut, unused_imports, unused_variables)]

use anyhow::Result;
use for_mqtt_client::{
    protocol::MqttOptions, MqttEvent, ProtocolV4, QoS
};
use log::{
    debug, error, info, warn,
    LevelFilter::{Debug, Info}
};
use std::{io::Read, time::Duration};
use tokio::{spawn, time::sleep};

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    let _ = custom_utils::logger::custom_build(Debug)
        // .module("for_event_bus::bus", Info)
        // .module("for_mqtt_client::tasks::task_publish", Debug)
        .build_default()
        .log_to_stdout()
        .start();
    let mut options = MqttOptions::new(
        "abc111sfew".to_string(),
        "broker.emqx.io".to_string(),
        1883
    )?;

    let (client, mut rx) = options
        .set_keep_alive(30)
        .auto_reconnect()
        .connect_to_v4()
        .await?;
    spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event.as_ref() {
                MqttEvent::ConnectSuccess(session_present) => {
                    info!("\nConnectSuccess {}\n", session_present);
                },
                MqttEvent::ConnectFail(reason) => {
                    info!("\nConnectFail：{} \n", reason);
                },
                MqttEvent::Publish(packet) => {
                    info!(
                        "\nRx Publish：{:x?} \n",
                        packet.payload.as_ref()
                    );
                },
                MqttEvent::PublishSuccess(id) => {
                    info!("\nPublish Success：{} \n", id);
                },
                MqttEvent::SubscribeAck(ack) => {
                    info!("\nSubscribeAck：{:?} \n", ack);
                },
                MqttEvent::UnsubscribeAck(ack) => {
                    info!("\nUnsubscribeAck：{:?} \n", ack)
                },
                event => {
                    info!("\nMqttEvent：{:?} \n", event);
                }
            }
        }
        warn!("**************");
    });
    println!(
        "{:?}",
        client
            .to_subscribe("abcfew".to_string(), QoS::ExactlyOnce)
            .await
    );
    sleep(Duration::from_secs(5)).await;
    info!(
        "{:?}",
        client
            .publish(
                "abcfew".to_string(),
                QoS::AtMostOnce,
                "abc".as_bytes(),
                false
            )
            .await?
    );
    sleep(Duration::from_secs(15)).await;
    client.unsubscribe("abcfew".to_string()).await?;
    sleep(Duration::from_secs(9000)).await;
    client.disconnect().await?;
    sleep(Duration::from_secs(120)).await;
    Ok(())
}
