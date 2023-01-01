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
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::{select, spawn};

use crate::tasks::task_connect::{Connected, TaskConnect};
use crate::tasks::task_hub::data::{KeepAliveTime, Reason, State};
use crate::tasks::task_ping::TaskPing;
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
        network_task.run();

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
        let mut keep_alive = KeepAliveTime::init();
        loop {
            if self.state.is_connected() {
                self._run(&mut a, &mut b, &mut keep_alive).await;
            } else {
                self.try_to_connect(&mut keep_alive).await;
            }
        }
    }

    async fn _run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        keep_alive_time: &mut KeepAliveTime,
    ) {
        loop {
            if !self.state.is_connected() {
                return;
            }
            select! {
                hub_msg = self.rx.recv() => match hub_msg {
                    Some(msg) => self.deal_hub_msg(msg, a, b, keep_alive_time).await,
                    None => todo!()
                }
            }
        }
    }
    ///
    async fn try_to_connect(&mut self, keep_alive_time: &mut KeepAliveTime) {
        debug!("try to connect");
        loop {
            TaskConnect::init(
                self.options.clone(),
                self.senders.clone(),
                self.tx_for_connect.clone(),
            );
            match self.rx_from_connect.recv().await {
                Some(req) => {
                    self.state = State::Connected;

                    self.init_keep_alive_check(keep_alive_time);
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
        keep_alive_time: &mut KeepAliveTime,
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
                self.init_keep_alive_check(keep_alive_time);
            }
            HubMsg::Error => {}
            HubMsg::PingSuccess => {
                self.state = State::Connected;
                self.init_keep_alive_check(keep_alive_time);
            }
            HubMsg::PingFail => {
                self.state = State::UnConnected(Reason::PingFail);
                todo!()
            }
            HubMsg::KeepAlive(keep_alive) => {
                if *keep_alive_time == keep_alive {
                    debug!("send ping req");
                    TaskPing::init(self.senders.clone());
                }
            }
        }
    }
    /// 初始化一个keep alive的计时
    fn init_keep_alive_check(&self, keep_alive_time: &mut KeepAliveTime) {
        debug!("init_keep_alive_check");
        let keep_alive = keep_alive_time.update();
        init_keep_alive_check(
            keep_alive,
            self.options.keep_alive(),
            self.senders.tx_hub.clone(),
        );
    }
}

fn init_keep_alive_check(time: KeepAliveTime, keep_alive: u16, tx: mpsc::Sender<HubMsg>) {
    spawn(async move {
        sleep(Duration::from_secs(keep_alive as u64)).await;
        debug!("keep_alive_check wake {:?}", time);
        if tx.send(HubMsg::KeepAlive(time)).await.is_err() {
            error!("fail to send keep alive check");
        }
    });
}
