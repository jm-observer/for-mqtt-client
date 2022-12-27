use crate::tasks::{Receipt, Receipter};
use bytes::Bytes;
use std::ops::Deref;
use std::sync::Arc;

#[derive(Debug)]
pub struct NetworkData {
    pub(crate) data: Arc<Bytes>,
    pub(crate) receipter: Receipter,
}

impl NetworkData {
    pub fn init(data: Arc<Bytes>, receipter: Receipter) -> Self {
        Self { data, receipter }
    }
    pub fn done(self) {
        self.receipter.done();
    }
}

impl Deref for NetworkData {
    type Target = Arc<Bytes>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
