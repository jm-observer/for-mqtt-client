use std::time::Duration;
use anyhow::Result;
use for_mqtt_client::v3_1_1::MqttOptions;

#[tokio::main]
async fn main() -> Result<()> {
    custom_utils::logger::logger_stdout_debug();
    let options = MqttOptions::new("abc", "broker.emqx.io", 1883);
    let client = options.run().await;
    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}