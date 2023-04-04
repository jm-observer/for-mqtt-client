#![allow(dead_code, unused_mut, unused_imports, unused_variables)]

use anyhow::Result;
use for_mqtt_client::protocol::MqttOptions;
use for_mqtt_client::tls::TlsConfig;
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
        .build_default()
        .log_to_stdout()
        .start();

    let config =
        TlsConfig::default().set_server_ca_pem_file("resources/broker.emqx.io-ca.crt".into());
    let options = MqttOptions::new("abc111sfew".to_string(), "54.87.92.106".to_string(), 8883)?
        .set_keep_alive(30)
        .auto_reconnect()
        .set_tls(config);

    let _client = options.connect_to_v4().await;
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
    println!(
        "{:?}",
        _client
            .to_subscribe("abcfew".to_string(), QoS::ExactlyOnce)
            .await
    );
    sleep(Duration::from_secs(5)).await;
    info!(
        "{:?}",
        _client
            .publish(
                "abcfew".to_string(),
                QoS::AtMostOnce,
                "abc".as_bytes(),
                false
            )
            .await?
    );
    sleep(Duration::from_secs(15)).await;
    _client.unsubscribe("abcfew".to_string()).await?;
    sleep(Duration::from_secs(90)).await;
    _client.disconnect().await?;
    sleep(Duration::from_secs(120)).await;
    Ok(())
}