mod unsubscribe;

use crate::datas::id::Id;
use crate::protocol::packet::subscribe::{RetainForwardRule, Subscribe};
use crate::protocol::packet::{write_mqtt_bytes, write_mqtt_string, write_remaining_length};
use crate::protocol::PropertyType;
use crate::{Protocol, ProtocolV5, QoS, TraceSubscribe};
use anyhow::bail;
use bytes::{BufMut, Bytes, BytesMut};
use std::marker::PhantomData;

pub struct SubscribeBuilder<T: Protocol> {
    pub trace_id: Id,
    pub id: Option<SubscribeId>,
    pub user_properties: Vec<(String, String)>,
    pub filters: Vec<FilterBuilder<T>>,
}

impl<T: Protocol> SubscribeBuilder<T> {
    pub fn add_filter(&mut self, path: String, qos: QoS) -> &mut FilterBuilder<T> {
        self.filters.push(FilterBuilder::new(path, qos));
        let index = self.filters.len() - 1;
        unsafe { self.filters.get_unchecked_mut(index) }
    }
}
impl SubscribeBuilder<ProtocolV5> {
    pub fn add_user_properties(&mut self, key: String, val: String) -> &mut Self {
        self.user_properties.push((key, val));
        self
    }
    pub fn set_id(&mut self, id: SubscribeId) -> &mut Self {
        self.id = Some(id);
        self
    }
}

pub struct SubscribeId {
    datas: Bytes,
}

impl SubscribeId {
    pub fn new(id: u32) -> anyhow::Result<Self> {
        if id > 268_435_455u32 && id == 0 {
            bail!("todo")
        }
        let mut buffer = BytesMut::new();
        write_remaining_length(&mut buffer, id as usize);
        Ok(Self {
            datas: buffer.freeze(),
        })
    }
}

pub struct FilterBuilder<T: Protocol> {
    path: String,
    qos: QoS,
    no_local: bool,
    preserve_retain: bool,
    retain_forward_rule: RetainForwardRule,
    protocol: PhantomData<T>,
}

impl<T: Protocol> FilterBuilder<T> {
    pub fn new(path: String, qos: QoS) -> Self {
        Self {
            path,
            qos,
            no_local: false,
            preserve_retain: false,
            retain_forward_rule: Default::default(),
            protocol: Default::default(),
        }
    }

    pub fn build(self) -> SubscribeBuilder<T> {
        SubscribeBuilder {
            trace_id: Default::default(),
            id: None,
            user_properties: vec![],
            filters: vec![self],
        }
    }
}
impl FilterBuilder<ProtocolV5> {
    pub fn set_nolocal(&mut self, no_local: bool) -> &mut Self {
        self.no_local = no_local;
        self
    }
    pub fn set_preserve_retain(&mut self, preserve_retain: bool) -> &mut Self {
        self.preserve_retain = preserve_retain;
        self
    }
    pub fn set_retain_forward_rule(&mut self, retain_forward_rule: RetainForwardRule) -> &mut Self {
        self.retain_forward_rule = retain_forward_rule;
        self
    }
}

impl<T: Protocol> From<SubscribeBuilder<T>> for TraceSubscribe {
    fn from(value: SubscribeBuilder<T>) -> Self {
        let SubscribeBuilder {
            trace_id,
            id,
            user_properties,
            filters,
        } = value;

        let subscribe = if T::is_v4() {
            let mut buffer = BytesMut::new();
            for filter in filters {
                write_filter(filter, &mut buffer)
            }

            Subscribe::V4 {
                packet_id: 0,
                payload: buffer.freeze(),
            }
        } else {
            let mut buffer = BytesMut::new();
            for filter in filters {
                write_filter(filter, &mut buffer)
            }

            let properties_datas = write_properties(id, user_properties);
            // let mut buffer_properties = BytesMut::with_capacity(properties_datas.len() + 2);
            // buffer_properties.put_u16(properties_datas.len() as u16);
            // write_mqtt_bytes(&mut buffer_properties, properties_datas.as_ref());
            Subscribe::V5 {
                packet_id: 0,
                properties: properties_datas,
                filters: buffer.freeze(),
            }
        };
        TraceSubscribe {
            id: trace_id.0,
            subscribe,
        }
    }
}
fn write_properties(id: Option<SubscribeId>, user_properties: Vec<(String, String)>) -> Bytes {
    let mut buffer = BytesMut::new();
    if let Some(id) = id {
        buffer.put_u8(PropertyType::SubscriptionIdentifier as u8);
        write_mqtt_bytes(&mut buffer, id.datas.as_ref());
    }
    for (key, value) in user_properties.iter() {
        buffer.put_u8(PropertyType::UserProperty as u8);
        write_mqtt_string(&mut buffer, key);
        write_mqtt_string(&mut buffer, value);
    }
    buffer.freeze()
}
fn write_filter<T: Protocol>(value: FilterBuilder<T>, buffer: &mut BytesMut) {
    if T::is_v4() {
        let FilterBuilder { path, qos, .. } = value;
        let options = qos as u8;

        write_mqtt_string(buffer, path.as_str());
        buffer.put_u8(options);
    } else {
        let FilterBuilder {
            path,
            qos,
            no_local,
            preserve_retain,
            retain_forward_rule,
            protocol: _,
        } = value;
        let mut options = qos as u8;
        if no_local {
            options |= 1 << 2;
        }
        if preserve_retain {
            options |= 1 << 3;
        }
        retain_forward_rule.merge_to_u8(&mut options);

        write_mqtt_string(buffer, path.as_str());
        buffer.put_u8(options);
    }
}
