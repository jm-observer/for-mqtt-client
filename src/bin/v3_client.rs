#![allow(dead_code, unused_mut, unused_imports, unused_variables)]
use anyhow::Result;
use for_mqtt_client::v3_1_1::MqttOptions;
use for_mqtt_client::QoS;
use log::{debug, error};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let mut options = MqttOptions::new("abc111".to_string(), "broker.emqx.io".to_string(), 1883);
    options.set_keep_alive(30);
    let (_client, mut event_rx) = options.run().await;
    sleep(Duration::from_secs(1330)).await;
    Ok(())
}
