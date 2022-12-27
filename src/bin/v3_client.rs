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
    let options = MqttOptions::new("abc", "broker.emqx.io", 1883);
    let (_client, mut event_rx) = options.run().await;

    debug!("client");
    _client.subscribe("/tmp/me".to_string(), QoS::AtLeastOnce);
    debug!("subscribe");
    // tokio::spawn(async move {
    //     while let Ok(event) = event_rx.recv().await {
    //         debug!("{:?}", event);
    //     }
    //     error!("error");
    // });

    sleep(Duration::from_secs(60)).await;
    Ok(())
}
