mod data;

use crate::tasks::task_network::{Data, NetworkEvent, TaskNetwork};
use crate::tasks::{BroadcastTx, Senders};
use crate::v3_1_1::{qos, Connect, MqttOptions, Publish};
use anyhow::{anyhow, Result};
use log::{debug, error, warn};
use ringbuf::{Consumer, Producer};
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{channel, Receiver};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::{select, spawn};

use crate::tasks::task_client::data::MqttEvent;
use crate::tasks::task_client::Client;
use crate::tasks::task_hub::data::{HubState, KeepAliveTime, Reason, State};
use crate::tasks::task_ping::TaskPing;
use crate::tasks::task_publish::{
    TaskPublishQos0, TaskPublishQos1, TaskPublishQos2, TaskPublishRxQos1, TaskPublishRxQos2,
};
use crate::tasks::task_subscribe::{TaskSubscribe, TaskUnsubscribe};
use crate::{ClientCommand, QoS, QoSWithPacketId, TracePublish};
pub use data::HubMsg;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

pub struct TaskHub {
    options: MqttOptions,
    senders: Senders,
    state: HubState,
    rx: mpsc::Receiver<HubMsg>,
    rx_from_client: mpsc::Receiver<ClientCommand>,
    rx_from_network: mpsc::Receiver<NetworkEvent>,
    rx_publish: HashMap<u16, Publish>,
    rx_publish_id: HashMap<u16, u16>,
}
impl TaskHub {
    pub async fn connect(options: MqttOptions) -> (Client, Receiver<MqttEvent>) {
        let (tx_network_write, rx_network_writer) = mpsc::channel(1024);
        let (tx_hub, rx_hub) = mpsc::channel(1024);
        let (tx_for_network, rx_from_network) = mpsc::channel(1024);
        let (tx_for_client, rx_from_client) = mpsc::channel(1024);

        let senders = Senders::init(tx_network_write, tx_hub, tx_for_client);

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
            state: HubState::default(),
            rx_from_network,
            rx_publish: HashMap::default(),
            rx_publish_id: Default::default(),
            rx_from_client,
        };
        let client = Client::init(senders);
        let event_rx = client.init_receiver();
        network_task.run();

        spawn(async move {
            // todo
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
            match self.state {
                HubState::ToConnect => {
                    self.run_to_connect(&mut keep_alive).await;
                }
                HubState::Connected => {
                    self.run_connected(&mut a, &mut b, &mut keep_alive).await?;
                }
                HubState::ToStop => {
                    todo!()
                }
                HubState::Stoped => {
                    todo!()
                }
            }
        }
    }

    async fn run_connected(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        keep_alive_time: &mut KeepAliveTime,
    ) -> Result<()> {
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                hub_msg = self.rx.recv() => match hub_msg {
                    Some(msg) => self.deal_hub_msg(msg, a, b, keep_alive_time).await,
                    None => todo!()
                },
                client_command = self.rx_from_client.recv() => {
                    self.deal_client_command(client_command.ok_or(anyhow!("rx_from_client.recv none"))?, b).await?;
                },
                network_status = self.rx_from_network.recv() => match network_status {
                    Some(network_status) => {
                        self.update_state_by_network_status(network_status.into());
                    },
                    None => todo!()
                }
            }
        }
    }
    fn update_state_by_network_status(&mut self, state: NetworkEvent) {
        debug!("update_state: {:?}", state);
        self.state = self.state.update_by_network_status(&state);
        match state {
            NetworkEvent::Connected => {
                self.senders.tx_to_user(MqttEvent::ConnectSuccess);
            }
            NetworkEvent::Disconnect(msg) => {
                self.senders.tx_to_user(MqttEvent::ConnectFail(msg));
            }
            _ => {
                todo!()
            }
        }
    }
    fn update_state_by_ping(&mut self, is_success: bool) {
        self.state = self.state.update_by_ping(is_success);
    }
    ///
    async fn run_to_connect(&mut self, keep_alive_time: &mut KeepAliveTime) {
        debug!("try to connect");
        loop {
            if self.state.is_to_connect() {
                match self.rx_from_network.recv().await {
                    Some(status) => {
                        self.update_state_by_network_status(status.into());
                    }
                    None => {
                        error!("");
                    }
                }
            } else {
                return;
            }
        }
    }

    async fn wait_for_client_command(&mut self, keep_alive_time: &mut KeepAliveTime) {
        todo!()
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
                self.update_state_by_ping(true);
                self.init_keep_alive_check(keep_alive_time);
            }
            HubMsg::PingFail => {
                // 需要再看看文档，看如何处理
                todo!();
                // self.update_state_by_ping(false);
                // if self.senders.tx_network.send(Data::Reconnect).await.is_err() {
                //     error!("")
                // }
            }
            HubMsg::KeepAlive(keep_alive) => {
                if *keep_alive_time == keep_alive {
                    debug!("send ping req");
                    TaskPing::init(self.senders.clone());
                }
            }
            HubMsg::RxPublish(publish) => match publish.qos {
                QoSWithPacketId::AtMostOnce => {
                    self.senders.tx_to_user(publish);
                }
                QoSWithPacketId::AtLeastOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        self.rx_publish_id.insert(id, id);
                        TaskPublishRxQos1::init(self.senders.clone(), id);
                    }
                }
                QoSWithPacketId::ExactlyOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        self.rx_publish_id.insert(id, id);
                        TaskPublishRxQos2::init(self.senders.clone(), id);
                    }
                }
            },
            HubMsg::AffirmRxId(id) => {
                if self.rx_publish_id.remove(&id).is_none() {
                    error!("todo")
                }
            }
            HubMsg::AffirmRxPublish(id) => {
                if let Some(publish) = self.rx_publish.remove(&id) {
                    self.senders.tx_to_user(publish.clone());
                } else {
                    error!("todo")
                }
            }
        }
    }
    async fn deal_client_command(
        &mut self,
        req: ClientCommand,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<()> {
        match req {
            ClientCommand::Subscribe(trace_subscribe) => match self.state {
                HubState::ToConnect => {
                    todo!()
                }
                HubState::Connected => {
                    TaskSubscribe::init(
                        self.senders.clone(),
                        trace_subscribe,
                        self.request_id(b).await?,
                    );
                }
                HubState::ToStop | HubState::Stoped => {
                    todo!()
                }
            },
            ClientCommand::Publish(trace_publish) => match self.state {
                HubState::ToConnect => {
                    todo!()
                }
                HubState::Connected => {
                    self.publish_client_msg(trace_publish, b).await?;
                }
                HubState::ToStop | HubState::Stoped => {
                    todo!()
                }
            },
            ClientCommand::Unsubscribe(trace_unsubscribe) => match self.state {
                HubState::ToConnect => {
                    todo!()
                }
                HubState::Connected => {
                    TaskUnsubscribe::init(
                        self.senders.clone(),
                        trace_unsubscribe,
                        self.request_id(b).await?,
                    );
                }
                HubState::ToStop | HubState::Stoped => {
                    todo!()
                }
            },
            ClientCommand::Connect => {
                self.update_to_connect().await;
            }
            ClientCommand::Disconnect => {
                self.update_to_stop().await;
            }
        }
        Ok(())
    }
    async fn update_to_connect(&mut self) {
        self.state = HubState::ToConnect;
        // todo 启动相关任务
    }
    async fn update_to_stop(&mut self) {
        self.state = HubState::ToStop;
        // todo 停止相关任务
    }
    async fn request_id(&mut self, b: &mut Consumer<u16, Arc<SharedRb>>) -> Result<u16> {
        if let Some(id) = b.pop() {
            debug!("request id: {}", id);
            return Ok(id);
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
            self.senders.tx_hub_msg.clone(),
        );
    }

    async fn publish_client_msg(
        &mut self,
        trace_publish: TracePublish,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<()> {
        match trace_publish.qos {
            QoS::AtMostOnce => {
                TaskPublishQos0::init(self.senders.clone(), trace_publish).await;
            }
            QoS::AtLeastOnce => {
                let packet_id = self.request_id(b).await?;
                TaskPublishQos1::init(self.senders.clone(), trace_publish, packet_id).await;
            }
            QoS::ExactlyOnce => {
                let packet_id = self.request_id(b).await?;
                TaskPublishQos2::init(self.senders.clone(), trace_publish, packet_id).await;
            }
        }
        Ok(())
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
