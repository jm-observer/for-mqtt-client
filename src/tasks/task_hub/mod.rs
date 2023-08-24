mod data;
mod unacknowledged;

pub use unacknowledged::*;

use crate::tasks::{
    task_network::{HubNetworkCommand, NetworkEvent, TaskNetwork},
    Senders,
};
use anyhow::Result;
use for_event_bus::{
    upcast, BusEvent, EntryOfBus, Event, IdentityOfRx,
    IdentityOfSimple, IdentityOfTx, SimpleBus, ToWorker, Worker,
};
use log::{debug, error, info, warn};
use ringbuf::{Consumer, Producer};
use std::{
    collections::{HashMap, VecDeque},
    mem::MaybeUninit,
    sync::Arc,
    time::Duration,
};
use tokio::{select, spawn, time::sleep};

use crate::{
    protocol::{
        packet::{Connect, Publish},
        MqttOptions, Protocol,
    },
    tasks::{
        task_client::{data::MqttEvent, Client, ClientRx},
        task_ping::TaskPing,
        task_publish::{
            TaskPublishQos0, TaskPublishQos1, TaskPublishQos2,
            TaskPublishRxQos1, TaskPublishRxQos2,
        },
        task_subscribe::{TaskSubscribe, TaskUnsubscribe},
    },
    ClientCommand, ClientData, QoSWithPacketId,
};
pub use data::*;

type SharedRb = ringbuf::SharedRb<u16, Vec<MaybeUninit<u16>>>;

#[derive(Worker)]
pub struct TaskHub {
    protocol: Protocol,
    options: MqttOptions,
    state: HubState,
    bus: EntryOfBus,
    identity: IdentityOfRx,
    identity_command: IdentityOfSimple<ClientCommand>,
    identity_network: IdentityOfSimple<NetworkEvent>,
    // tx_to_user: Sender<MqttEvent>,
    rx_publish: HashMap<u16, Publish>,
    rx_publish_id: HashMap<u16, u16>,
    client_data: VecDeque<UnacknowledgedClientData>, /* rx_client_data: mpsc::Receiver<ClientData>,
                                                      * rx_client_command: mpsc::Receiver<ClientCommand>, */
}

impl TaskHub {
    pub async fn connect(
        options: MqttOptions,
        protocol: Protocol,
    ) -> Result<(Client, ClientRx), HubError> {
        let bus = SimpleBus::init();
        let identity = bus.login::<Self>().await?;
        identity.subscribe::<HubMsg>().await?;
        identity.subscribe::<ClientData>().await?;
        let identity_command =
            bus.simple_login::<Self, ClientCommand>().await?;
        let identity_network =
            bus.simple_login::<Self, NetworkEvent>().await?;
        let client = Client::init(protocol, bus.clone()).await?;

        let mut hub = Self {
            options,
            state: HubState::default(),
            rx_publish: HashMap::default(),
            rx_publish_id: Default::default(),
            client_data: Default::default(),
            protocol,
            bus,
            identity,
            identity_command,
            identity_network,
        };

        spawn(async move {
            let (mut a, mut b) =
                ringbuf::SharedRb::new(65535).split();
            for i in 1..=u16::MAX {
                if let Err(e) = a.push(i) {
                    error!("push {} fail", e);
                    return;
                }
            }
            loop {
                if let Err(e) = hub.run(&mut a, &mut b).await {
                    error!("{:?}", e);
                }
                if hub.state.is_disconnected() {
                    debug!("hub close");
                    return;
                } else {
                    debug!("hub try to run");
                }
            }
        });
        Ok(client)
    }

    async fn run(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        // let (mut senders, mut rx_hub_msg, mut rx_hub_network_event)
        // = (None, None, None);
        loop {
            debug!("{:?}", self.state);
            match &mut self.state {
                HubState::ToConnect => {
                    self.run_to_connect().await?;
                    if self.state.is_connected() {
                        for data in self.client_data.iter() {
                            data.to_acknowledge(&self.bus).await?;
                        }
                    }
                },
                HubState::Connected => {
                    self.run_connected(a, b).await?;
                },
                HubState::ToDisconnect(_reason) => {
                    self.identity
                        .dispatch_event(HubNetworkCommand::Disconnect)
                        .await?;
                    match _reason {
                        ToDisconnectReason::PingFail => {
                            if self.options.auto_reconnect {
                                self.state = HubState::ToConnect;
                            } else {
                                self.state = HubState::Disconnected;
                            }
                        },
                        ToDisconnectReason::ClientCommand => {
                            self.state = HubState::Disconnected;
                        },
                        ToDisconnectReason::NetworkErr(_) => {
                            if self.options.auto_reconnect {
                                self.state = HubState::ToConnect;
                            } else {
                                self.state = HubState::Disconnected;
                            }
                        },
                    }
                },
                HubState::Disconnected => return Ok(()),
            }
        }
    }

    async fn run_connected(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        debug!("run_connected");
        loop {
            if !self.state.is_connected() {
                return Ok(());
            }
            // self.recv(a, b).await?;
            select! {
                command = self.identity_command.recv() => {
                    self.deal_client_command_when_connected(command?.as_ref()).await?
                },
                network_status = self.identity_network.recv() => {
                    self.update_connected_state_by_network_status(network_status?.as_ref()).await?
                },
                data = self.identity.recv_event() => {
                    self.recv(a, b, data?).await?;
                },
            }
        }
    }

    pub async fn recv(
        &mut self,
        a: &mut Producer<u16, Arc<SharedRb>>,
        b: &mut Consumer<u16, Arc<SharedRb>>,
        event: BusEvent,
    ) -> Result<(), HubError> {
        // if let Ok(command) =
        // event.clone().downcast::<ClientCommand>() {
        //     self.deal_client_command_when_connected(command.
        // as_ref())         .await?
        // } else
        if let Ok(command) =
            upcast(event.clone()).downcast::<ClientData>()
        {
            self.deal_client_data_when_connected(
                command.as_ref().clone(),
                b,
            )
            .await?
        // } else if let Ok(network_status) =
        // event.clone().downcast::<NetworkEvent>() {
        //     self.update_connected_state_by_network_status(network_status.as_ref())?
        } else if let Ok(hub_msg) = upcast(event).downcast::<HubMsg>()
        {
            self.deal_hub_msg(hub_msg.as_ref(), a, b).await?
        } else {
            return Err(HubError::Other(
                "should not be reached".to_string(),
            ));
        }
        Ok(())
    }

    async fn update_connected_state_by_network_status(
        &mut self,
        state: &NetworkEvent,
    ) -> Result<(), HubError> {
        debug!("update_state: {:?}", state);
        match state {
            NetworkEvent::ConnectedErr(msg) => {
                // self.tx_to_user.send(MqttEvent::ConnectedErr(msg.
                // clone()))?;
                self.identity
                    .dispatch_event(MqttEvent::ConnectedErr(
                        msg.clone(),
                    ))
                    .await?;

                self.state = HubState::ToDisconnect(
                    ToDisconnectReason::NetworkErr(msg.clone()),
                );
                // if self.options.auto_reconnect {
                //     self.state = HubState::ToConnect;
                // } else {
                //     self.state = HubState::Disconnected;
                // }
            },
            NetworkEvent::Connected(_)
            | NetworkEvent::ConnectFail(_) => {
                warn!(
                    "should not rx NetworkEvent::Disconnected | \
                     Connected | ConnectFail when connected"
                )
            },
            NetworkEvent::BrokerDisconnect(packet) => {
                self.identity
                    .dispatch_event(MqttEvent::ConnectedErr(format!(
                        "{:?}",
                        packet
                    )))
                    .await?;
                if self.options.auto_reconnect {
                    self.state = HubState::ToConnect;
                } else {
                    self.state = HubState::Disconnected;
                }
            },
        }
        Ok(())
    }

    ///
    async fn run_to_connect(
        &mut self,
    ) -> Result<(), HubToConnectError> {
        let mut first = true;
        loop {
            if !first {
                sleep(Duration::from_secs(30)).await;
            } else {
                first = false;
            }
            self.try_deal_client_command_when_to_connect().await?;
            if !self.state.is_to_connect() {
                return Ok(());
            }
            // let (
            //     senders,
            //     rx_hub_msg,
            //     mut rx_hub_network_event,
            //     rx_network_data,
            //     rx_hub_network_command,
            // ) = Senders::init();
            let (addr, port) = self.options.broker_address();
            TaskNetwork::init(
                addr,
                port,
                Connect::new(&self.options, self.protocol).map_err(
                    |x| HubToConnectError::Other(x.to_string()),
                )?,
                self.protocol.clone(),
                self.options.network_protocol.clone(),
                &self.bus,
            )
            .await?
            .run();
            debug!("try to connect");
            let status = self.identity_network.recv().await?;
            match status.as_ref() {
                NetworkEvent::Connected(session_present) => {
                    debug!("Connected");
                    self.state = HubState::Connected;
                    self.init_keep_alive_check();

                    self.identity
                        .dispatch_event(MqttEvent::ConnectSuccess(
                            *session_present,
                        ))
                        .await?;

                    // self.tx_to_user
                    //     .send(MqttEvent::ConnectSuccess(session_present))?;
                    return Ok(());
                },
                NetworkEvent::ConnectedErr(reason) => {
                    warn!(
                        "should not to rx \
                         NetworkEvent::Disconnect({}) when \
                         to_connect",
                        reason
                    );
                },
                NetworkEvent::ConnectFail(reason) => {
                    info!("connect fail: {:?}", reason);
                    self.identity
                        .dispatch_event(MqttEvent::ConnectFail(
                            reason.clone(),
                        ))
                        .await?;

                    if self.options.auto_reconnect {
                        continue;
                    } else {
                        self.state = HubState::Disconnected;
                        return Err(
                            HubToConnectError::ChannelAbnormal,
                        );
                    }
                },
                NetworkEvent::BrokerDisconnect(packet) => {
                    info!("connect fail: {:?}", packet);

                    self.identity
                        .dispatch_event(MqttEvent::ConnectedErr(
                            format!("{:?}", packet),
                        ))
                        .await?;

                    if self.options.auto_reconnect {
                        continue;
                    } else {
                        self.state = HubState::Disconnected;
                        return Err(
                            HubToConnectError::ChannelAbnormal,
                        );
                    }
                },
            }
        }
    }

    async fn deal_hub_msg(
        &mut self,
        req: &HubMsg,
        a: &mut Producer<u16, Arc<SharedRb>>,
        _b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        match req {
            HubMsg::RecoverId(id) => {
                debug!("recover id: {}", id);
                let Some((index, obj)) = self
                    .client_data
                    .iter_mut()
                    .enumerate()
                    .find(|(_index, obj)| {
                        obj.packet_id() == *id
                    }) else
                {
                    return Ok(())
                };
                if obj.acknowledge() {
                    self.client_data.remove(index);
                    self.init_keep_alive_check();
                    a.push(*id).map_err(|_x| {
                        HubError::PacketIdErr(
                            "RecoverId Err".to_string(),
                        )
                    })?;
                } else {
                    obj.to_acknowledge(&self.bus).await?;
                }
            },
            HubMsg::PingSuccess => {
                self.init_keep_alive_check();
            },
            HubMsg::PingFail => {
                // 需要再看看文档，看如何处理
                self.state = HubState::ToDisconnect(
                    ToDisconnectReason::PingFail,
                );
                self.identity
                    .dispatch_event(MqttEvent::ConnectedErr(
                        "ping fail".to_string(),
                    ))
                    .await?;
            },
            HubMsg::KeepAlive(keep_alive) => {
                if keep_alive.latest() {
                    debug!("send ping req");
                    TaskPing::init(self.bus.clone()).await?;
                }
            },
            HubMsg::RxPublish(publish) => match publish.qos {
                QoSWithPacketId::AtMostOnce => {
                    // self.tx_to_user.send(publish.into())?;
                    self.identity
                        .dispatch_event(MqttEvent::Publish(
                            publish.clone(),
                        ))
                        .await?;
                },
                QoSWithPacketId::AtLeastOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!(
                            "rx dup publish {:?} from broker",
                            publish
                        )
                    } else {
                        self.rx_publish.insert(id, publish.clone());
                        self.rx_publish_id.insert(id, id);
                        TaskPublishRxQos1::init(
                            self.bus.clone(),
                            id,
                            self.protocol,
                        )
                        .await?;
                    }
                },
                QoSWithPacketId::ExactlyOnce(id) => {
                    if self.rx_publish_id.contains_key(&id) {
                        debug!(
                            "rx dup publish {:?} from broker",
                            publish
                        )
                    } else {
                        TaskPublishRxQos2::init(
                            self.bus.clone(),
                            id,
                            publish.protocol,
                        )
                        .await?;
                        self.rx_publish.insert(id, publish.clone());
                        self.rx_publish_id.insert(id, id);
                    }
                },
            },
            HubMsg::AffirmRxId(id) => {
                if self.rx_publish_id.remove(&id).is_none() {
                    warn!("could not AffirmRxId {}", id);
                }
            },
            HubMsg::AffirmRxPublish(id) => {
                if let Some(publish) = self.rx_publish.remove(&id) {
                    self.identity
                        .dispatch_event(MqttEvent::Publish(publish))
                        .await?;
                    // self.tx_to_user.send(publish.into())?;
                } else {
                    warn!("could not AffirmRxPublish {}", id);
                }
            },
        }
        Ok(())
    }

    async fn deal_client_data_when_connected(
        &mut self,
        req: ClientData,
        b: &mut Consumer<u16, Arc<SharedRb>>,
    ) -> Result<(), HubError> {
        match req {
            ClientData::Subscribe(mut trace_subscribe) => {
                trace_subscribe.set_packet_id(b).await?;
                self.client_data
                    .push_back(trace_subscribe.clone().into());
                TaskSubscribe::init(
                    self.bus.clone(),
                    trace_subscribe.clone(),
                )
                .await?;
            },
            ClientData::Unsubscribe(mut trace_unsubscribe) => {
                trace_unsubscribe.set_packet_id(b).await?;
                self.client_data
                    .push_back(trace_unsubscribe.clone().into());
                TaskUnsubscribe::init(
                    self.bus.clone(),
                    trace_unsubscribe.clone(),
                )
                .await?;
            },
            ClientData::PublishQoS0(packet) => {
                TaskPublishQos0::init(
                    Senders::init(self.identity.tx()),
                    packet.clone(),
                )
                .await;
            },
            ClientData::PublishQoS1(mut packet) => {
                packet.set_packet_id(b).await?;
                self.client_data.push_back(packet.clone().into());
                TaskPublishQos1::init(self.bus.clone(), packet)
                    .await?;
            },
            ClientData::PublishQoS2(mut packet) => {
                packet.set_packet_id(b).await?;
                self.client_data.push_back(packet.clone().into());
                TaskPublishQos2::init(self.bus.clone(), packet)
                    .await?;
            },
        }
        Ok(())
    }

    async fn deal_client_command_when_connected(
        &mut self,
        command: &ClientCommand,
    ) -> Result<(), HubError> {
        debug!("deal_client_command_when_connected: {:?}", command);
        match command {
            ClientCommand::DisconnectAndDrop => {
                self.state = HubState::ToDisconnect(
                    ToDisconnectReason::ClientCommand,
                );
            },
            ClientCommand::ViolenceDisconnectAndDrop => {
                self.state = HubState::Disconnected;
                return Err(HubError::ViolenceDisconnectAndDrop);
            },
        }
        Ok(())
    }

    /// 初始化一个keep alive的计时
    fn init_keep_alive_check(&self) {
        debug!("init_keep_alive_check");
        init_keep_alive_check(
            KeepAliveTime::default(),
            self.options.keep_alive(),
            self.identity.tx(),
        );
    }

    /// bool: if rx command
    async fn try_deal_client_command_when_to_connect(
        &mut self,
    ) -> Result<(), HubToConnectError> {
        loop {
            if let Some(command) = self.identity_command.try_recv()? {
                match command.as_ref() {
                    ClientCommand::DisconnectAndDrop => {
                        self.state = HubState::Disconnected;
                    },
                    ClientCommand::ViolenceDisconnectAndDrop => {
                        self.state = HubState::Disconnected;
                        return Err(HubToConnectError::ViolenceDisconnectAndDrop);
                    },
                }
            } else {
                break;
            }
        }
        Ok(())
    }
}

fn init_keep_alive_check(
    time: KeepAliveTime,
    keep_alive: u16,
    tx: IdentityOfTx,
) {
    spawn(async move {
        sleep(Duration::from_secs(keep_alive as u64)).await;
        debug!("keep_alive_check wake {:?}", time);
        if tx.dispatch_event(HubMsg::KeepAlive(time)).await.is_err() {
            error!("fail to send HubMsg::KeepAlive")
        }
    });
}
