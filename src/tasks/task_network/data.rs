use std::ops::Deref;
use std::sync::Arc;
use bytes::Bytes;
use crate::tasks::{Receipt, Receipter};

#[derive(Debug)]
pub struct NetworkData {
    data: Arc<Bytes>,
    receipter: Receipter
}

impl NetworkData {
    pub fn init(data: Arc<Bytes>,
                receipter: Receipter) -> Self {
        Self {
            data, receipter
        }
    }
    pub fn done(self) {
        self.receipter.done();
    }

}

impl Deref for NetworkData{
    type Target = Arc<Bytes>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}