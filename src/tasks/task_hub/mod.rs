mod data;
mod unacknowledged;

pub use unacknowledged::*;

use crate::tasks::task_network::{HubNetworkCommand, NetworkEvent, TaskNetwork};
use crate::tasks::Senders;
use anyhow::Result;
use log::{debug, error, info, warn};
use ringbuf::{Consumer, Producer};
use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::{channel, Sender};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::sleep;
use tokio::{select, spawn};

use crate::protocol::packet::Connect;
use crate::protocol::packet::Publish;
use crate::protocol::{MqttOptions, Protocol};
use crate::tasks::task_client::data::MqttEvent;
use crate::tasks::task_client::Client;
use crate::tasks::task_ping::TaskPing;
use crate::tasks::task_publish::{
    TaskPublishQos0, TaskPublishQos1, TaskPublishQos2, TaskPublishRxQos1, TaskPublishRxQos2,
};
use crate::tasks::task_subscribe::{TaskSubscribe, TaskUnsubscribe};
use crate::{ClientCommand, ClientData, QoSWithPacketId};
pub use data::*;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

pub struct TaskHub {
    protocol: Protocol,
    options: MqttOptions,
    state: HubState,
    tx_to_user: Sender<MqttEvent>,
    rx_publish: HashMap<u16, Publish>,
    rx_publish_id: HashMap<u16, u16>,
    client_data: VecDeque<UnacknowledgedClientData>,
    rx_client_data: mpsc::Receiver<ClientData>,
    rx_client_command: mpsc::Receiver<ClientCommand>,
}
impl TaskHub {
    pub async fn connect(options: MqttOptions, protocol: Protocol) -> Client {
        let (tx_client_data, rx_client_data) = mpsc::channel(1024);
        let (tx_client_command, rx_client_command) = mpsc::channel(1024);
        let (tx_to_user, _) = channel(1024);
        let client = Client::init(
            tx_client_data,
            tx_client_command,
            tx_to_user.clone(),
            protocol,
        );
        // let client = Client::init(
        //     tx_client_data,
        //     tx_client_command,
        //     tx_to_user.clone(),
        //     options.protocol,
        // );
        let mut hub = Self {
            options,
            state: HubState::default(),
            rx_publish: HashMap::default(),
            rx_publish_id: Default::default(),
            rx_client_data,
            rx_client_command,
            tx_to_user,
            client_data: Default::default(),
            protocol,
        };
        let _event_rx = client.init_receiver();

        spawn(async move {
            let (mut a, mut b) = ringbuf::SharedRb::new(65535).split();
            for i in 1..=u16::MAX {
                let _ = a.push(i);
            }
            if let Err(e) = hub.run(&mut a, &mut b).await {
                error!("{:?}", e);
            }
        });
        client
    }
    async fn run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        let (mut senders, mut rx_hub_msg, mut rx_hub_network_event) = (None, None, None);
        loop {
            debug!("{:?}", self.state);
            match &mut self.state {
                HubState::ToConnect => match self.run_to_connect().await? {
                    Some(vals) => {
                        senders = Some(vals.0);
                        rx_hub_msg = Some(vals.1);
                        rx_hub_network_event = Some(vals.2);
                    }
                    None => continue,
                },
                HubState::Connected => {
                    match (&mut senders, &mut rx_hub_msg, &mut rx_hub_network_event) {
                        (Some(senders), Some(rx_hub_msg), Some(rx_hub_network_event)) => {
                            self.run_connected(a, b, rx_hub_msg, rx_hub_network_event, &senders)
                                .await?;
                        }
                        _ => {
                            return Err(HubError::StateErr(format!(
                                "senders/rx_hub_msg/rx_hub_network_event should not be none"
                            )));
                        }
                    }
                }
                HubState::ToDisconnect(_reason) => {
                    rx_hub_msg.take();
                    rx_hub_network_event.take();
                    if let Some(senders) = senders.take() {
                        senders
                            .tx_hub_network_command
                            .send(HubNetworkCommand::Disconnect)
                            .await?;
                        self.state = HubState::Disconnected;
                    } else {
                        return Err(HubError::StateErr(format!("senders should not be none")));
                    }
                }
                HubState::Disconnected => return Ok(()),
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
    ) -> Result<(), HubError> {
        debug!("run_connected");
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            select! {
                hub_msg = rx_hub_msg.recv() =>  {
                    self.deal_hub_msg(hub_msg.ok_or(HubError::ChannelAbnormal)?, a, b, &senders).await?;
                },
                client_command = self.rx_client_command.recv() => {
                    self.deal_client_command_when_connected(client_command.ok_or(HubError::ChannelAbnormal)?).await?;
                },
                client_data = self.rx_client_data.recv() => {
                    self.deal_client_data_when_connected(client_data.ok_or(HubError::ChannelAbnormal)?, b, &senders).await?;
                },
                network_status = rx_network_event.recv() => {
                    self.update_connected_state_by_network_status(network_status.ok_or(HubError::ChannelAbnormal)?)?;
                }
            }
        }
    }
    fn update_connected_state_by_network_status(
        &mut self,
        state: NetworkEvent,
    ) -> Result<(), HubError> {
        debug!("update_state: {:?}", state);
        match state {
            NetworkEvent::ConnectedErr(msg) => {
                self.tx_to_user.send(MqttEvent::ConnectedErr(msg))?;
                if self.options.auto_reconnect {
                    self.state = HubState::ToConnect;
                } else {
                    self.state = HubState::Disconnected;
                }
            }
            NetworkEvent::Connected(_) | NetworkEvent::ConnectFail(_) => {
                warn!("should not rx NetworkEvent::Disconnected | Connected | ConnectFail when connected")
            }
            NetworkEvent::BrokerDisconnect(packet) => {
                // todo
                self.tx_to_user
                    .send(MqttEvent::ConnectedErr(format!("{:?}", packet)))?;
                if self.options.auto_reconnect {
                    self.state = HubState::ToConnect;
                } else {
                    self.state = HubState::Disconnected;
                }
            }
        }
        Ok(())
    }
    ///
    async fn run_to_connect(
        &mut self,
    ) -> Result<
        Option<(
            Senders,
            mpsc::Receiver<HubMsg>,
            mpsc::Receiver<NetworkEvent>,
        )>,
        HubToConnectError,
    > {
        loop {
            self.try_deal_client_command_when_to_connect().await?;
            if !self.state.is_to_connect() {
                return Ok(None);
            }
            let (
                senders,
                rx_hub_msg,
                mut rx_hub_network_event,
                rx_network_data,
                rx_hub_network_command,
            ) = Senders::init(1024, self.tx_to_user.clone());
            let (addr, port) = self.options.broker_address();
            TaskNetwork::init(
                addr,
                port,
                senders.clone(),
                rx_network_data,
                Connect::new(&self.options, self.protocol)
                    .map_err(|x| HubToConnectError::Other(x.to_string()))?,
                rx_hub_network_command,
                self.protocol.clone(),
                self.options.network_protocol.clone(),
            )
            .run();
            debug!("try to connect");
            let status = rx_hub_network_event
                .recv()
                .await
                .ok_or(HubToConnectError::ChannelAbnormal)?;
            match status {
                NetworkEvent::Connected(session_present) => {
                    debug!("Connected");
                    self.state = HubState::Connected;
                    self.init_keep_alive_check(&senders);
                    self.tx_to_user
                        .send(MqttEvent::ConnectSuccess(session_present))?;
                    for data in self.client_data.iter() {
                        data.to_acknowledge(&senders).await;
                    }
                    return Ok(Some((senders, rx_hub_msg, rx_hub_network_event)));
                }
                NetworkEvent::ConnectedErr(reason) => {
                    warn!(
                        "should not to rx NetworkEvent::Disconnect({}) when to_connect",
                        reason
                    )
                }
                NetworkEvent::ConnectFail(reason) => {
                    info!("connect fail: {:?}", reason);
                    self.tx_to_user.send(MqttEvent::ConnectFail(reason))?;
                    if self.options.auto_reconnect {
                        continue;
                    } else {
                        self.state = HubState::Disconnected;
                        return Err(HubToConnectError::ChannelAbnormal);
                    }
                }
                NetworkEvent::BrokerDisconnect(packet) => {
                    info!("connect fail: {:?}", packet);
                    self.tx_to_user
                        .send(MqttEvent::ConnectedErr(format!("{:?}", packet)))?;
                    if self.options.auto_reconnect {
                        continue;
                    } else {
                        self.state = HubState::Disconnected;
                        return Err(HubToConnectError::ChannelAbnormal);
                    }
                }
            }
        }
    }

    async fn deal_hub_msg(
        &mut self,
        req: HubMsg,
        a: &mut Producer<u16, Arc<SharedRb>>,
        _b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<(), HubError> {
        match req {
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                let Some((index, obj)) = self
                    .client_data
                    .iter_mut()
                    .enumerate()
                    .find(|(_index, obj)| {
                        obj.packet_id() == id
                    }) else
                {
                    return Ok(())
                };
                if obj.acknowledge() {
                    self.client_data.remove(index);
                    self.init_keep_alive_check(senders);
                    a.push(id)
                        .map_err(|_x| HubError::PacketIdErr("RecoverId Err".to_string()))?;
                } else {
                    obj.to_acknowledge(senders).await;
                }
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
                        TaskPublishRxQos1::init(senders.clone(), id, self.protocol);
                    }
                }
                QoSWithPacketId::ExactlyOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!("rx dup publish {:?} from broker", publish)
                    } else {
                        TaskPublishRxQos2::init(senders.clone(), id, publish.protocol);
                        self.rx_publish.insert(id, publish);
                        self.rx_publish_id.insert(id, id);
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
    async fn deal_client_data_when_connected(
        &mut self,
        req: ClientData,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        senders: &Senders,
    ) -> Result<(), HubError> {
        match req {
            ClientData::Subscribe(mut trace_subscribe) => {
                trace_subscribe.set_packet_id(b).await?;
                self.client_data.push_back(trace_subscribe.clone().into());
                TaskSubscribe::init(senders.clone(), trace_subscribe);
            }
            ClientData::Unsubscribe(mut trace_unsubscribe) => {
                trace_unsubscribe.set_packet_id(b).await?;
                self.client_data.push_back(trace_unsubscribe.clone().into());
                TaskUnsubscribe::init(senders.clone(), trace_unsubscribe);
            }
            ClientData::PublishQoS0(packet) => {
                TaskPublishQos0::init(senders.clone(), packet).await;
            }
            ClientData::PublishQoS1(mut packet) => {
                packet.set_packet_id(b).await?;
                self.client_data.push_back(packet.clone().into());
                TaskPublishQos1::init(senders.clone(), packet).await;
            }
            ClientData::PublishQoS2(mut packet) => {
                packet.set_packet_id(b).await?;
                self.client_data.push_back(packet.clone().into());
                TaskPublishQos2::init(senders.clone(), packet).await;
            }
        }
        Ok(())
    }
    async fn deal_client_command_when_connected(
        &mut self,
        command: ClientCommand,
    ) -> Result<(), HubError> {
        debug!("deal_client_command_when_connected: {:?}", command);
        match command {
            ClientCommand::DisconnectAndDrop => {
                self.state = HubState::ToDisconnect(ToDisconnectReason::ClientCommand);
            }
            ClientCommand::ViolenceDisconnectAndDrop => {
                self.state = HubState::Disconnected;
                return Err(HubError::ViolenceDisconnectAndDrop);
            }
        }
        Ok(())
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

    /// bool: if rx command
    async fn try_deal_client_command_when_to_connect(&mut self) -> Result<(), HubToConnectError> {
        loop {
            match self.rx_client_command.try_recv() {
                Ok(command) => {
                    debug!("try_deal_client_command_when_to_connect: {:?}", command);
                    match command {
                        ClientCommand::DisconnectAndDrop => {
                            self.state = HubState::Disconnected;
                        }
                        ClientCommand::ViolenceDisconnectAndDrop => {
                            self.state = HubState::Disconnected;
                            return Err(HubToConnectError::ViolenceDisconnectAndDrop);
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Err(HubToConnectError::ChannelAbnormal),
            }
        }
        Ok(())
    }
}

fn init_keep_alive_check(time: KeepAliveTime, keep_alive: u16, tx: mpsc::Sender<HubMsg>) {
    spawn(async move {
        sleep(Duration::from_secs(keep_alive as u64)).await;
        debug!("keep_alive_check wake {:?}", time);
        let _ = tx.send(HubMsg::KeepAlive(time)).await;
    });
}
