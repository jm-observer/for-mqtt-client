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

use crate::tasks::task_connect::{Connected, TaskConnect};
use crate::tasks::task_hub::data::{Reason, State};
pub use data::HubMsg;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

pub struct TaskHub {
    options: MqttOptions,
    senders: Senders,
    state: State,
    rx: mpsc::Receiver<HubMsg>,
    tx_for_connect: mpsc::Sender<Connected>,
    rx_from_connect: mpsc::Receiver<Connected>,
}
impl TaskHub {
    pub async fn init(options: MqttOptions) -> (Client, Receiver<MqttEvent>) {
        let (tx_user, _) = channel(1024);
        let (tx_network_write, rx_network_writer) = mpsc::channel(1024);
        let (tx_publish, rx_publisher) = mpsc::channel(1024);
        let (tx_subscribe, _) = channel(1024);
        let (tx_ping, _) = channel(1024);
        let (tx_connect, _) = channel(1024);

        let (tx_hub, rx_hub) = mpsc::channel(1024);
        let (tx_for_connect, rx_from_connect) = mpsc::channel(1024);

        let senders = Senders::init(
            tx_network_write,
            tx_publish,
            tx_subscribe,
            tx_user,
            tx_hub,
            tx_ping,
            tx_connect,
        );

        let (addr, port) = options.broker_address();
        let network_task = TaskNetwork::init(
            addr.parse().unwrap(),
            port,
            senders.clone(),
            rx_network_writer,
        );
        let hub = Self {
            options,
            senders: senders.clone(),
            rx: rx_hub,
            state: State::default(),
            rx_from_connect,
            tx_for_connect,
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
                self.try_to_connect().await;
            }
        }
    }

    async fn _run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) {
        loop {
            if !self.state.is_connected() {
                return;
            }
            select! {
                hub_msg = self.rx.recv() => match hub_msg {
                    Some(msg) => self.deal_hub_msg(msg, a, b).await,
                    None => todo!()
                }
            }
        }
    }
    ///
    async fn try_to_connect(&mut self) {
        loop {
            TaskConnect::init(
                self.options.clone(),
                self.senders.clone(),
                self.tx_for_connect.clone(),
            );
            match self.rx_from_connect.recv().await {
                Some(req) => {
                    self.state = State::Connected;
                    break;
                }
                None => {
                    error!("");
                }
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
            HubMsg::PingSuccess => {
                self.state = State::Connected;
            }
            HubMsg::PingFail => {
                self.state = State::UnConnected(Reason::PingFail);
                todo!()
            }
        }
    }
}
