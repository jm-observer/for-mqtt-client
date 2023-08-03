use anyhow::Result;
use for_mqtt_client::{
    protocol::MqttOptions, tls::TlsConfig, MqttEvent, QoS,
};
use log::{info, warn, LevelFilter::Debug};
use std::time::Duration;
use tokio::{spawn, time::sleep};

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    custom_utils::logger::custom_build(Debug)
        .build_default()
        .log_to_stdout()
        .start();

    let config = TlsConfig::default().insecurity();
    let options = MqttOptions::new(
        "mq-id".to_string(),
        "broker.emqx.io".to_string(),
        8883,
    )?
    .set_keep_alive(30)
    .set_credentials("afe".to_string(), "fe4yl".to_string())
    .auto_reconnect()
    .set_tls(config);

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
                },
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
    sleep(Duration::from_secs(90)).await;
    client.disconnect().await?;
    sleep(Duration::from_secs(120)).await;
    Ok(())
}
