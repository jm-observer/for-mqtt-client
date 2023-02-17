mod data;

use crate::tasks::task_network::{Data, NetworkEvent, TaskNetwork};
use crate::tasks::{BroadcastTx, Senders};
use crate::v3_1_1::{qos, Connect, MqttOptions, Publish};
use anyhow::{anyhow, bail, Context, Result};
use log::{debug, error, warn};
use ringbuf::{Consumer, Producer};
use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::{select, spawn};

use crate::tasks::task_client::data::MqttEvent;
use crate::tasks::task_client::Client;
use crate::tasks::task_hub::data::{HubState, KeepAliveTime, Reason, State, ToDisconnectReason};
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
    tx_publish: VecDeque<TracePublish>,
    rx_from_client: mpsc::Receiver<ClientCommand>,
}
impl TaskHub {
    pub async fn connect(options: MqttOptions) -> Client {
        let (tx_of_client, rx_from_client) = mpsc::channel(1024);
        let (tx_to_user, _) = channel(1024);
        let client = Client::init(tx_of_client, tx_to_user.clone());
        let mut hub = Self {
            options,
            state: HubState::default(),
            rx_publish: HashMap::default(),
            rx_publish_id: Default::default(),
            rx_from_client,
            tx_to_user,
            tx_publish: Default::default(),
        };
        let event_rx = client.init_receiver();

        spawn(async move {
            let (mut a, mut b) = ringbuf::SharedRb::new(65535).split();
            for i in 1..=u16::MAX {
                a.push(i).unwrap();
            }
            loop {
                if let Err(e) = hub.run(&mut a, &mut b).await {
                    // todo!() 清理连接的参数等等？
                    hub.tx_to_user
                        .send(MqttEvent::ConnectFail(e.to_string()))
                        .unwrap();
                    sleep(Duration::from_secs(5)).await;
                } else {
                    error!("unreadch!!");
                }
            }
        });
        client
    }
    async fn run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<()> {
        let (mut senders, mut rx_hub_msg, mut rx_hub_network_event) = (None, None, None);
        loop {
            match &mut self.state {
                HubState::ToConnect => {
                    let vals = self.run_to_connect().await?;
                    senders = Some(vals.0);
                    rx_hub_msg = Some(vals.1);
                    rx_hub_network_event = Some(vals.2);
                }
                HubState::Connected => {
                    match (&mut senders, &mut rx_hub_msg, &mut rx_hub_network_event) {
                        (Some(senders), Some(rx_hub_msg), Some(rx_hub_network_event)) => {
                            self.run_connected(a, b, rx_hub_msg, rx_hub_network_event, &senders)
                                .await?;
                        }
                        _ => {
                            bail!("senders/rx_hub_msg/rx_hub_network_event should not be none");
                        }
                    }
                }
                HubState::ToDisconnect(reason) => {
                    rx_hub_msg.take();
                    rx_hub_network_event.take();
                    if let Some(senders) = senders.take() {
                        senders.tx_network_data.send(Data::Disconnect).await?;
                        self.state = HubState::Disconnected;
                    } else {
                        bail!("senders should not be none");
                    }
                }
                HubState::Disconnected => {
                    let command = self.rx_from_client.recv().await.ok_or(anyhow!("todo"))?;
                    self.deal_client_command_when_disconnected(command).await?;
                }
            }
        }
    }

    async fn run_connected(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        rx_hub_msg: &mut mpsc::Receiver<HubMsg>,
        rx_network_event: &mut mpsc::Receiver<NetworkEvent>,
        senders: &Senders,
    ) -> Result<()> {
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                hub_msg = rx_hub_msg.recv() =>  {
                    self.deal_hub_msg(hub_msg.ok_or(anyhow!("todo"))?, a, b, &senders).await?;
                },
                client_command = self.rx_from_client.recv() => {
                    self.deal_client_command_when_connected(client_command.ok_or(anyhow!("rx_from_client.recv none"))?, b, &senders).await?;
                },
                network_status = rx_network_event.recv() => {
                        self.update_connected_state_by_network_status(network_status.ok_or(anyhow!("todo"))?.into())?;
                }
            }
        }
    }
    fn update_connected_state_by_network_status(&mut self, state: NetworkEvent) -> Result<()> {
        debug!("update_state: {:?}", state);
        match state {
            NetworkEvent::Disconnect(msg) => {
                self.state = HubState::ToDisconnect(ToDisconnectReason::NetworkError(msg));
            }
            NetworkEvent::Disconnected | NetworkEvent::Connected => {
                warn!("should not rx NetworkEvent::Disconnected | NetworkEvent::Connected when connected")
            }
        }
        Ok(())
    }
    ///
    async fn run_to_connect(
        &mut self,
    ) -> Result<(
        Senders,
        mpsc::Receiver<HubMsg>,
        mpsc::Receiver<NetworkEvent>,
    )> {
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
                    Some(status) => match status {
                        NetworkEvent::Connected => {
                            self.state = HubState::Connected;
                        }
                        NetworkEvent::Disconnected => {}
                        NetworkEvent::Disconnect(_) => {}
                    },
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

    async fn deal_hub_msg(
        &mut self,
        req: HubMsg,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<()> {
        match req {
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                a.push(id).map_err(|x| anyhow!("致命错误，todo"))?;
                let Some(obj) = self.tx_publish.iter().enumerate().find(|(index, obj) | obj.packet_id() == id) else {
                    bail!("could find packet id = {:?}", id);
                };
                self.tx_publish.remove(obj.0);
                self.init_keep_alive_check(senders);
            }
            HubMsg::PingSuccess => {
                self.init_keep_alive_check(senders);
            }
            HubMsg::PingFail => {
                // 需要再看看文档，看如何处理
                self.state = HubState::ToDisconnect(ToDisconnectReason::PingFail);
            }
            HubMsg::KeepAlive(keep_alive) => {
                if keep_alive.latest() {
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
                    warn!("could not AffirmRxId {}", id);
                }
            }
            HubMsg::AffirmRxPublish(id) => {
                if let Some(publish) = self.rx_publish.remove(&id) {
                    self.tx_to_user.send(publish.into())?;
                } else {
                    warn!("could not AffirmRxPublish {}", id);
                }
            }
        }
        Ok(())
    }
    async fn deal_client_command_when_connected(
        &mut self,
        req: ClientCommand,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<()> {
        match req {
            ClientCommand::Subscribe(trace_subscribe) => {
                TaskSubscribe::init(senders.clone(), trace_subscribe, self.request_id(b).await?);
            }
            ClientCommand::Publish(trace_publish) => {
                self.publish_client_msg(trace_publish, b, senders).await?;
            }
            ClientCommand::Unsubscribe(trace_unsubscribe) => {
                TaskUnsubscribe::init(
                    senders.clone(),
                    trace_unsubscribe,
                    self.request_id(b).await?,
                );
            }
            ClientCommand::ReConnect(options) => {
                self.options = options;
                self.state = HubState::ToConnect;
            }
            ClientCommand::Disconnect => {
                self.state = HubState::ToDisconnect(ToDisconnectReason::ClientCommand);
            }
        }
        Ok(())
    }
    async fn deal_client_command_when_disconnected(&mut self, req: ClientCommand) -> Result<()> {
        match req {
            ClientCommand::Subscribe(trace_subscribe) => {
                self.tx_to_user.send(MqttEvent::SubscribeFail(
                    "client had disconneced".to_string(),
                ))?;
            }
            ClientCommand::Publish(trace_publish) => {
                self.tx_to_user
                    .send(MqttEvent::PublishFail("client had disconneced".to_string()))?;
            }
            ClientCommand::Unsubscribe(trace_unsubscribe) => {
                self.tx_to_user.send(MqttEvent::UnsubscribeFail(
                    "client had disconneced".to_string(),
                ))?;
            }
            ClientCommand::ReConnect(options) => {
                self.options = options;
                self.state = HubState::ToConnect;
            }
            ClientCommand::Disconnect => {
                self.tx_to_user.send(MqttEvent::Disconnected)?;
            }
        }
        Ok(())
    }
    async fn request_id(&mut self, b: &mut Consumer<u16, Arc<SharedRb>>) -> Result<u16> {
        if let Some(id) = b.pop() {
            debug!("request id: {}", id);
            return Ok(id);
        } else {
            bail!("buffer of packet id is empty");
        }
    }
    /// 初始化一个keep alive的计时
    fn init_keep_alive_check(&self, senders: &Senders) {
        debug!("init_keep_alive_check");
        init_keep_alive_check(
            KeepAliveTime::default(),
            self.options.keep_alive(),
            senders.tx_hub_msg.clone(),
        );
    }

    async fn publish_client_msg(
        &mut self,
        mut trace_publish: TracePublish,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<()> {
        match trace_publish.qos {
            QoS::AtMostOnce => {
                TaskPublishQos0::init(senders.clone(), trace_publish).await;
            }
            QoS::AtLeastOnce => {
                let packet_id = self.request_id(b).await?;
                trace_publish.set_packet_id(packet_id);
                self.tx_publish.push_back(trace_publish.clone());
                TaskPublishQos1::init(senders.clone(), trace_publish, packet_id).await;
            }
            QoS::ExactlyOnce => {
                let packet_id = self.request_id(b).await?;
                trace_publish.set_packet_id(packet_id);
                self.tx_publish.push_back(trace_publish.clone());
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
