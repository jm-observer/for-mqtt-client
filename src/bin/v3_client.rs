#![allow(dead_code, unused_mut, unused_imports, unused_variables)]

use anyhow::Result;
use for_mqtt_client::v3_1_1::MqttOptions;
use for_mqtt_client::QoS;
use log::{debug, error};
use std::io::Read;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut options = MqttOptions::new("abc111".to_string(), "broker.emqx.io".to_string(), 1883);
    options.set_keep_alive(30);
    let (_client, mut event_rx) = options.run().await;
    _client
        .subscribe("abcfew".to_string(), QoS::AtMostOnce)
        .await;
    sleep(Duration::from_secs(2)).await;
    // _client
    //     .publish(
    //         "abcfew".to_string(),
    //         QoS::AtMostOnce,
    //         "abc".as_bytes().into(),
    //         false,
    //     )
    //     .await;
    _client
        .publish(
            "abcfew11".to_string(),
            QoS::AtLeastOnce,
            "abc".as_bytes().into(),
            false,
        )
        .await;
    _client
        .publish(
            "eeeeee".to_string(),
            QoS::ExactlyOnce,
            "eeeeeee".as_bytes().into(),
            false,
        )
        .await;
    _client.unsubscribe("abcfew11".to_string()).await;
    sleep(Duration::from_secs(1330)).await;
    Ok(())
}
