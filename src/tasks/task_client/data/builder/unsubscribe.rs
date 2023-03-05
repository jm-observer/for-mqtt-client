use crate::datas::id::Id;

use crate::protocol::packet::unsubscribe::Unsubscribe;
use crate::protocol::packet::write_mqtt_string;
use crate::protocol::PropertyType;
use crate::{Protocol, ProtocolV5, TraceUnubscribe};

use bytes::{BufMut, Bytes, BytesMut};
use std::marker::PhantomData;

pub struct UnsubscribeBuilder<T: Protocol> {
    pub trace_id: Id,
    pub user_properties: Vec<(String, String)>,
    pub filters: Vec<UnsubscribeFilterBuilder<T>>,
}

impl<T: Protocol> UnsubscribeBuilder<T> {
    pub fn add_filter(&mut self, path: String) -> &mut UnsubscribeFilterBuilder<T> {
        self.filters.push(UnsubscribeFilterBuilder::new(path));
        let index = self.filters.len() - 1;
        unsafe { self.filters.get_unchecked_mut(index) }
    }
}
impl UnsubscribeBuilder<ProtocolV5> {
    pub fn add_user_properties(&mut self, key: String, val: String) -> &mut Self {
        self.user_properties.push((key, val));
        self
    }
}

pub struct UnsubscribeFilterBuilder<T: Protocol> {
    path: String,
    protocol: PhantomData<T>,
}

impl<T: Protocol> UnsubscribeFilterBuilder<T> {
    pub fn new(path: String) -> Self {
        Self {
            path,
            protocol: Default::default(),
        }
    }

    pub fn build(self) -> UnsubscribeBuilder<T> {
        UnsubscribeBuilder {
            trace_id: Default::default(),
            user_properties: vec![],
            filters: vec![self],
        }
    }
}

impl<T: Protocol> From<UnsubscribeBuilder<T>> for TraceUnubscribe {
    fn from(value: UnsubscribeBuilder<T>) -> Self {
        let UnsubscribeBuilder {
            trace_id,
            user_properties,
            filters,
        } = value;

        let unsubscribe = if T::is_v4() {
            let mut buffer = BytesMut::new();
            for filter in filters {
                write_filter(filter, &mut buffer)
            }
            Unsubscribe::V4 {
                packet_id: 0,
                payload: buffer.freeze(),
            }
        } else {
            let mut buffer = BytesMut::new();
            for filter in filters {
                write_filter(filter, &mut buffer)
            }
            let properties_datas = write_properties(user_properties);
            Unsubscribe::V5 {
                packet_id: 0,
                properties: properties_datas,
                filters: buffer.freeze(),
            }
        };
        TraceUnubscribe {
            id: trace_id.0,
            unsubscribe,
        }
    }
}
fn write_properties(user_properties: Vec<(String, String)>) -> Bytes {
    let mut buffer = BytesMut::new();
    for (key, value) in user_properties.iter() {
        buffer.put_u8(PropertyType::UserProperty as u8);
        write_mqtt_string(&mut buffer, key);
        write_mqtt_string(&mut buffer, value);
    }
    buffer.freeze()
}
fn write_filter<T: Protocol>(value: UnsubscribeFilterBuilder<T>, buffer: &mut BytesMut) {
    let UnsubscribeFilterBuilder { path, .. } = value;
    write_mqtt_string(buffer, path.as_str());
}
