mod data;

use crate::tasks::task_network::NetworkData;
use crate::tasks::{InnerCommand, Receipt, Receipter, RxBroadcast, Senders};
use crate::v3_1_1::{Connect, MqttOptions};
use anyhow::Result;
use bytes::BytesMut;
use log::{debug, error};
use tokio::select;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

pub use data::*;

pub struct TaskConnector {
    tx: Senders,
    rx_broadcast: RxBroadcast,
    options: MqttOptions,
    rx: mpsc::Receiver<ConnectMsg>,
}

impl TaskConnector {
    pub fn init(tx: Senders, options: MqttOptions, rx: mpsc::Receiver<ConnectMsg>) -> Self {
        let rx_broadcast = tx.subscribe_broadcast();
        Self {
            tx,
            rx_broadcast,
            options,
            rx,
        }
    }
    pub async fn run(mut self) {
        tokio::spawn(async move {
            loop {
                select! {
                    command = self.rx_broadcast.recv() => match command {
                        Ok(command) => {
                            self.deal_inner_command(command).await.unwrap();
                        }
                        Err(_) => {}
                    },
                    msg = self.rx.recv() => match msg {
                        Some(msg) => {
                            self.deal_msg(msg).await.unwrap();
                        }
                        None => {

                        }
                    }
                }
            }
        });
    }
    async fn deal_msg(&mut self, msg: ConnectMsg) -> Result<()> {
        match msg {
            ConnectMsg::ConnAck(ack) => {
                if ack.code.is_success() {
                    debug!("connect success");
                } else {
                    todo!()
                }
            }
            ConnectMsg::PingResp => {}
            ConnectMsg::Error => {}
        }
        Ok(())
    }
    async fn deal_inner_command(&mut self, command: InnerCommand) -> Result<()> {
        match command {
            InnerCommand::NetworkConnectSuccess => {
                let connect = Connect::new(self.options.client_id().clone());
                let mut datas = BytesMut::with_capacity(1024);
                connect.write(&mut datas)?;
                let (receipter, rx) = Receipter::init(Receipt::None);
                self.tx
                    .tx_network_writer
                    .send(NetworkData::init(datas.freeze().into(), receipter))
                    .await
                    .unwrap();
                match rx.await {
                    Ok(_) => {
                        debug!("done");
                    }
                    Err(e) => {
                        error!("fail to receive receipt")
                    }
                }
            }
            InnerCommand::NetworkConnectFail(_) => {}
        }
        Ok(())
    }
}
