mod data;

use crate::tasks::task_network::{NetworkData, NetworkMsg, TaskNetwork};
use crate::tasks::{MqttEvent, Senders, UserMsg};
use crate::utils::Endpoint;
use crate::v3_1_1::{Client, Connect, MqttOptions};
use anyhow::Result;
use log::{debug, error, warn};
use ringbuf::{Consumer, Producer};
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::mpsc;

use crate::tasks::task_hub::data::{Reason, State};
pub use data::HubMsg;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

pub struct TaskHub {
    options: MqttOptions,
    senders: Senders,
    state: State,
    rx: mpsc::Receiver<HubMsg>,
    rx_from_network: mpsc::Receiver<NetworkMsg>,
}
impl TaskHub {
    pub async fn init(options: MqttOptions) -> (Client, Receiver<MqttEvent>) {
        let (tx_user, _) = channel(1024);
        let (tx_network_writer, rx_network_writer) = mpsc::channel(1024);
        let (tx_publisher, rx_publisher) = mpsc::channel(1024);
        let (tx_subscriber, _) = channel(1024);
        let (tx_hub, rx_hub) = mpsc::channel(1024);
        let (tx_to_hub, rx_from_network) = mpsc::channel(1024);

        let senders = Senders::init(
            tx_network_writer,
            tx_publisher,
            tx_subscriber,
            tx_user,
            tx_hub,
        );

        let (addr, port) = options.broker_address();
        let network_task = TaskNetwork::init(
            addr.parse().unwrap(),
            port,
            senders.clone(),
            rx_network_writer,
            tx_to_hub,
        );
        let hub = Self {
            options,
            senders: senders.clone(),
            rx: rx_hub,
            state: State::default(),
            rx_from_network,
        };
        let client = Client::init(senders);
        let event_rx = client.init_receiver();
        network_task.run().await;

        tokio::spawn(async move {
            hub.run().await.unwrap();
        });
        (client, event_rx)
    }
    async fn run(mut self) -> Result<()> {
        let (mut a, mut b) = ringbuf::SharedRb::new(65535).split();
        for i in 1..=u16::MAX {
            a.push(i).unwrap();
        }
        loop {
            if self.state.is_connected() {
                self._run(&mut a, &mut b).await;
            } else {
                self.connect().await;
            }
        }
    }
    async fn deal_connected_msg(&mut self, req: NetworkMsg) {
        match req {
            NetworkMsg::NetworkConnectFail(e) => {
                error!("{}", e);
                self.state = State::UnConnected(Reason::NetworkErr(e));
            }
            msg => {
                warn!("{:?}", msg);
            }
        }
    }
    async fn deal_hub_msg(
        &mut self,
        req: HubMsg,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) {
        match req {
            HubMsg::RequestId(req) => {
                if let Some(id) = b.pop() {
                    debug!("request id: {}", id);
                    req.send(id).unwrap();
                } else {
                    todo!()
                }
            }
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                a.push(id).unwrap();
            }
            HubMsg::Error => {}
        }
    }
    async fn _run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) {
        loop {
            select! {
                hub_msg = self.rx.recv() => match hub_msg {
                    Some(msg) => self.deal_hub_msg(msg, a, b).await,
                    None => todo!()
                },
                network_msg = self.rx_from_network.recv() => match network_msg{
                    Some(msg) => self.deal_connected_msg(msg).await,
                    None => todo!()
                }
            }
        }
    }
    async fn connect(&mut self) {
        loop {
            match self.rx_from_network.recv().await {
                Some(req) => match req {
                    NetworkMsg::ConnAck(ack) => {
                        if ack.code.is_success() {
                            debug!("connect success");
                            self.state = State::Connected;
                            break;
                        } else {
                            todo!()
                        }
                    }
                    NetworkMsg::PingResp => {}
                    NetworkMsg::NetworkConnectSuccess => {
                        let connect = Connect::new(self.options.client_id().clone()).unwrap();
                        let receipter = self.senders.tx_network_default(connect).await.unwrap();
                        match receipter.await {
                            Ok(_) => {
                                debug!("done");
                            }
                            Err(e) => {
                                error!("fail to receive receipt")
                            }
                        }
                    }
                    NetworkMsg::NetworkConnectFail(e) => {
                        error!("{}", e);
                    }
                },
                None => {}
            }
        }
    }
}
