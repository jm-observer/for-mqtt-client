mod data;

use crate::tasks::task_network::{Data, NetworkStaus, TaskNetwork};
use crate::tasks::{BroadcastTx, MqttEvent, Senders};
use crate::v3_1_1::{qos, Connect, MqttOptions, Publish};
use anyhow::Result;
use log::{debug, error};
use ringbuf::{Consumer, Producer};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{channel, Receiver};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::{select, spawn};

use crate::tasks::task_client::Client;
use crate::tasks::task_hub::data::{KeepAliveTime, Reason, State};
use crate::tasks::task_ping::TaskPing;
use crate::tasks::task_publish::{
    TaskPublishQos0, TaskPublishQos1, TaskPublishQos2, TaskPublishRxQos1, TaskPublishRxQos2,
};
use crate::tasks::task_subscribe::{TaskSubscribe, TaskUnsubscribe};
use crate::{QoS, QoSWithPacketId};
pub use data::HubMsg;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

pub struct TaskHub {
    options: MqttOptions,
    senders: Senders,
    state: State,
    rx: mpsc::Receiver<HubMsg>,
    rx_from_network: mpsc::Receiver<NetworkStaus>,
    rx_publish: HashMap<u16, Publish>,
}
impl TaskHub {
    pub async fn init(options: MqttOptions) -> (Client, Receiver<MqttEvent>) {
        let (tx_network_write, rx_network_writer) = mpsc::channel(1024);
        let (tx_hub, rx_hub) = mpsc::channel(1024);
        let (tx_for_network, rx_from_network) = mpsc::channel(1024);

        let senders = Senders::init(tx_network_write, tx_hub);

        let (addr, port) = options.broker_address();
        let network_task = TaskNetwork::init(
            addr.parse().unwrap(),
            port,
            senders.clone(),
            rx_network_writer,
            Arc::new(Connect::new(&options)),
            tx_for_network,
        );
        let hub = Self {
            options,
            senders: senders.clone(),
            rx: rx_hub,
            state: State::default(),
            rx_from_network,
            rx_publish: HashMap::default(),
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
                },
                network_status = self.rx_from_network.recv() => match network_status {
                    Some(network_status) => {
                        self.update_state(network_status.into());
                    },
                    None => todo!()
                }
            }
        }
    }
    fn update_state(&mut self, state: State) {
        debug!("update_state: {:?}", state);
        self.state = state;
    }
    ///
    async fn try_to_connect(&mut self, keep_alive_time: &mut KeepAliveTime) {
        debug!("try to connect");
        loop {
            match self.rx_from_network.recv().await {
                Some(status) => {
                    self.update_state(status.into());
                    if self.state.is_connected() {
                        self.init_keep_alive_check(keep_alive_time);
                        return;
                    }
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
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                a.push(id).unwrap();
                self.init_keep_alive_check(keep_alive_time);
            }
            HubMsg::PingSuccess => {
                self.state = State::Connected;
                self.init_keep_alive_check(keep_alive_time);
            }
            HubMsg::PingFail => {
                self.update_state(State::UnConnected(Reason::PingFail));
                if self.senders.tx_network.send(Data::Reconnect).await.is_err() {
                    error!("")
                }
            }
            HubMsg::KeepAlive(keep_alive) => {
                if *keep_alive_time == keep_alive {
                    debug!("send ping req");
                    TaskPing::init(self.senders.clone());
                }
            }
            HubMsg::Subscribe { topic, qos } => {
                TaskSubscribe::init(self.senders.clone(), topic, qos, self.request_id(b).await);
            }
            HubMsg::Publish(trace_publish) => match trace_publish.qos {
                QoS::AtMostOnce => {
                    TaskPublishQos0::init(self.senders.clone(), trace_publish).await;
                }
                QoS::AtLeastOnce => {
                    let pkid = self.request_id(b).await;
                    TaskPublishQos1::init(self.senders.clone(), trace_publish, pkid).await;
                }
                QoS::ExactlyOnce => {
                    let pkid = self.request_id(b).await;
                    TaskPublishQos2::init(self.senders.clone(), trace_publish, pkid).await;
                }
            },
            HubMsg::Unsubscribe { topic } => {
                TaskUnsubscribe::init(self.senders.clone(), topic, self.request_id(b).await);
            }
            HubMsg::Disconnect => {}
            HubMsg::RxPublish(publish) => match publish.qos {
                QoSWithPacketId::AtMostOnce => {
                    self.senders.tx_to_user(publish);
                }
                QoSWithPacketId::AtLeastOnce(id) => {
                    if self.rx_publish.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        TaskPublishRxQos1::init(self.senders.clone(), id);
                    }
                }
                QoSWithPacketId::ExactlyOnce(id) => {
                    if self.rx_publish.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        TaskPublishRxQos2::init(self.senders.clone(), id);
                    }
                }
            },
            HubMsg::RecoverRxId(id) => {
                if let Some(publish) = self.rx_publish.remove(&id) {
                    self.senders.tx_to_user(publish);
                } else {
                    error!("todo")
                }
            }
        }
    }
    async fn request_id(&mut self, b: &mut Consumer<u16, Arc<SharedRb>>) -> u16 {
        if let Some(id) = b.pop() {
            debug!("request id: {}", id);
            return id;
        } else {
            todo!()
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
