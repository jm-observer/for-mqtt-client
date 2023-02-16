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
use tokio::sync::broadcast::{channel, Receiver, Sender};
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
    state: HubState,
    tx_to_user: Sender<MqttEvent>,
    rx_publish: HashMap<u16, Publish>,
    rx_publish_id: HashMap<u16, u16>,
    rx_from_client: mpsc::Receiver<ClientCommand>,
}
impl TaskHub {
    pub async fn connect(options: MqttOptions) -> Client {
        let (tx_of_client, rx_from_client) = mpsc::channel(1024);
        let (tx_to_user, _) = channel(1024);
        let client = Client::init(tx_of_client, tx_to_user.clone());
        let hub = Self {
            options,
            state: HubState::default(),
            rx_publish: HashMap::default(),
            rx_publish_id: Default::default(),
            rx_from_client,
            tx_to_user,
        };
        let event_rx = client.init_receiver();

        spawn(async move {
            // todo
            hub.run().await.unwrap();
        });
        client
    }
    async fn run(mut self) -> Result<()> {
        let (mut a, mut b) = ringbuf::SharedRb::new(65535).split();
        for i in 1..=u16::MAX {
            a.push(i).unwrap();
        }
        let mut keep_alive = KeepAliveTime::init();
        let (senders, mut rx_hub_msg, mut rx_hub_network_event) =
            self.run_to_connect(&mut keep_alive).await?;
        loop {
            match self.state {
                HubState::ToConnect => {
                    self.run_to_connect(&mut keep_alive).await?;
                }
                HubState::Connected => {
                    self.run_connected(
                        &mut a,
                        &mut b,
                        &mut keep_alive,
                        &mut rx_hub_msg,
                        &mut rx_hub_network_event,
                        &senders,
                    )
                    .await?;
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
        rx_hub_msg: &mut mpsc::Receiver<HubMsg>,
        rx_network_event: &mut mpsc::Receiver<NetworkEvent>,
        senders: &Senders,
    ) -> Result<()> {
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                hub_msg = rx_hub_msg.recv() => match hub_msg {
                    Some(msg) => self.deal_hub_msg(msg, a, b, keep_alive_time, &senders).await?,
                    None => todo!()
                },
                client_command = self.rx_from_client.recv() => {
                    self.deal_client_command(client_command.ok_or(anyhow!("rx_from_client.recv none"))?, b, &senders).await?;
                },
                network_status = rx_network_event.recv() => match network_status {
                    Some(network_status) => {
                        self.update_state_by_network_status(network_status.into());
                    },
                    None => todo!()
                }
            }
        }
    }
    fn update_state_by_network_status(&mut self, state: NetworkEvent) -> Result<()> {
        debug!("update_state: {:?}", state);
        self.state = self.state.update_by_network_status(&state);
        match state {
            NetworkEvent::Connected => {
                self.tx_to_user.send(MqttEvent::ConnectSuccess)?;
            }
            NetworkEvent::Disconnect(msg) => {
                self.tx_to_user.send(MqttEvent::ConnectFail(msg))?;
            }
            _ => {
                todo!()
            }
        }
        Ok(())
    }
    fn update_state_by_ping(&mut self, is_success: bool) {
        self.state = self.state.update_by_ping(is_success);
    }
    ///
    async fn run_to_connect(
        &mut self,
        keep_alive_time: &mut KeepAliveTime,
    ) -> Result<(
        Senders,
        mpsc::Receiver<HubMsg>,
        mpsc::Receiver<NetworkEvent>,
    )> {
        // init network task
        let (senders, rx_hub_msg, mut rx_hub_network_event, rx_network_data) =
            Senders::init(1024, self.tx_to_user.clone());
        let (addr, port) = self.options.broker_address();
        let network_task = TaskNetwork::init(
            addr,
            port,
            senders.clone(),
            rx_network_data,
            Connect::new(&self.options),
        )
        .run();
        debug!("try to connect");
        loop {
            if self.state.is_to_connect() {
                match rx_hub_network_event.recv().await {
                    Some(status) => {
                        self.update_state_by_network_status(status.into())?;
                    }
                    None => {
                        error!("");
                    }
                }
            } else {
                break;
            }
        }
        Ok((senders, rx_hub_msg, rx_hub_network_event))
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
        senders: &Senders,
    ) -> Result<()> {
        match req {
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                a.push(id).unwrap();
                self.init_keep_alive_check(keep_alive_time, senders);
            }
            HubMsg::PingSuccess => {
                self.update_state_by_ping(true);
                self.init_keep_alive_check(keep_alive_time, senders);
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
                    TaskPing::init(senders.clone());
                }
            }
            HubMsg::RxPublish(publish) => match publish.qos {
                QoSWithPacketId::AtMostOnce => {
                    self.tx_to_user.send(publish.into())?;
                }
                QoSWithPacketId::AtLeastOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        self.rx_publish_id.insert(id, id);
                        TaskPublishRxQos1::init(senders.clone(), id);
                    }
                }
                QoSWithPacketId::ExactlyOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        self.rx_publish.insert(id, publish);
                        self.rx_publish_id.insert(id, id);
                        TaskPublishRxQos2::init(senders.clone(), id);
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
                    self.tx_to_user.send(publish.into())?;
                } else {
                    error!("todo")
                }
            }
        }
        Ok(())
    }
    async fn deal_client_command(
        &mut self,
        req: ClientCommand,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<()> {
        match req {
            ClientCommand::Subscribe(trace_subscribe) => match self.state {
                HubState::ToConnect => {
                    todo!()
                }
                HubState::Connected => {
                    TaskSubscribe::init(
                        senders.clone(),
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
                    self.publish_client_msg(trace_publish, b, senders).await?;
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
                        senders.clone(),
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
    fn init_keep_alive_check(&self, keep_alive_time: &mut KeepAliveTime, senders: &Senders) {
        debug!("init_keep_alive_check");
        let keep_alive = keep_alive_time.update();
        init_keep_alive_check(
            keep_alive,
            self.options.keep_alive(),
            senders.tx_hub_msg.clone(),
        );
    }

    async fn publish_client_msg(
        &mut self,
        trace_publish: TracePublish,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<()> {
        match trace_publish.qos {
            QoS::AtMostOnce => {
                TaskPublishQos0::init(senders.clone(), trace_publish).await;
            }
            QoS::AtLeastOnce => {
                let packet_id = self.request_id(b).await?;
                TaskPublishQos1::init(senders.clone(), trace_publish, packet_id).await;
            }
            QoS::ExactlyOnce => {
                let packet_id = self.request_id(b).await?;
                TaskPublishQos2::init(senders.clone(), trace_publish, packet_id).await;
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
