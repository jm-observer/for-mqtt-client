#![allow(dead_code, unused_mut, unused_imports, unused_variables)]

use anyhow::Result;
use for_mqtt_client::v3_1_1::MqttOptions;
use for_mqtt_client::MqttEvent;
use for_mqtt_client::QoS;
use log::LevelFilter::{Debug, Info};
use log::{debug, error, info, warn};
use std::io::Read;
use std::time::Duration;
use tokio::spawn;
use tokio::time::sleep;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    custom_utils::logger::custom_build(Info)
        .module("for_mqtt_client::tasks::task_network", Debug)
        .module("for_mqtt_client::tasks::task_publish", Debug)
        .build_default()
        .log_to_stdout()
        .start();
    let mut options = MqttOptions::new("abc111".to_string(), "broker.emqx.io".to_string(), 1883);
    options.set_keep_alive(30);
    let _client = options.connect().await;
    let mut event_rx = _client.init_receiver();
    spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                MqttEvent::ConnectSuccess => {
                    info!("\nConnectSuccess \n");
                }
                MqttEvent::ConnectFail(reason) => {
                    info!("\nConnectFail：{} \n", reason);
                }
                MqttEvent::Publish(packet) => {
                    info!("\nRx Publish：{:?} \n", packet);
                }
                MqttEvent::PublishSuccess(id) => {
                    info!("\nPublish Success：{} \n", id);
                }
                MqttEvent::SubscribeAck(ack) => {
                    info!("\nSubscribeAck：{:?} \n", ack);
                }
                MqttEvent::UnsubscribeAck(ack) => info!("\nUnsubscribeAck：{:?} \n", ack),
            }
        }
        warn!("**************");
    });
    println!(
        "{:?}",
        _client
            .subscribe("abcfew".to_string(), QoS::ExactlyOnce)
            .await
    );
    println!(
        "{:?}",
        _client
            .subscribe("abcfewfe".to_string(), QoS::ExactlyOnce)
            .await
    );
    println!(
        "{:?}",
        _client
            .subscribe("abcfewwewew".to_string(), QoS::ExactlyOnce)
            .await
    );
    sleep(Duration::from_secs(2)).await;
    info!(
        "{:?}",
        _client
            .publish(
                "abcfew".to_string(),
                QoS::ExactlyOnce,
                "abc".as_bytes(),
                false
            )
            .await
            .unwrap()
    );
    info!(
        "{:?}",
        _client
            .publish(
                "abcfew".to_string(),
                QoS::ExactlyOnce,
                "abc".as_bytes(),
                false
            )
            .await
            .unwrap()
    );
    info!(
        "{:?}",
        _client
            .publish(
                "abcfew".to_string(),
                QoS::ExactlyOnce,
                "abc".as_bytes(),
                false
            )
            .await
            .unwrap()
    );
    sleep(Duration::from_secs(2)).await;
    _client.unsubscribe("abcfew".to_string()).await;
    sleep(Duration::from_secs(2)).await;
    info!(
        "{:?}",
        _client
            .publish("abcfew".to_string(), QoS::ExactlyOnce, "abc", false)
            .await
            .unwrap()
    );
    // _client
    //     .publish(
    //         "abcfew11".to_string(),
    //         QoS::AtLeastOnce,
    //         "abc".as_bytes().into(),
    //         false,
    //     )
    //     .await;
    // _client
    //     .publish(
    //         "eeeeee".to_string(),
    //         QoS::ExactlyOnce,
    //         "eeeeeee".as_bytes().into(),
    //         false,
    //     )
    //     .await;

    sleep(Duration::from_secs(1330)).await;
    Ok(())
}
