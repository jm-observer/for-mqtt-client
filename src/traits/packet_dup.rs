use crate::v3_1_1::{PubRec, PubRel, Publish, Subscribe, Unsubscribe};
use bytes::{Bytes, BytesMut};
use std::sync::Arc;

pub trait PacketDup {
    fn data(&self) -> Bytes;
    fn dup_data(&mut self) -> Arc<Bytes>;
}

impl PacketDup for Publish {
    fn data(&self) -> Bytes {
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        bytes.freeze()
    }

    fn dup_data(&mut self) -> Arc<Bytes> {
        self.dup = true;
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        Arc::new(bytes.freeze())
    }
}
impl PacketDup for PubRel {
    fn data(&self) -> Bytes {
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        bytes.freeze()
    }

    fn dup_data(&mut self) -> Arc<Bytes> {
        Arc::new(self.data())
    }
}
impl PacketDup for PubRec {
    fn data(&self) -> Bytes {
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        bytes.freeze()
    }

    fn dup_data(&mut self) -> Arc<Bytes> {
        Arc::new(self.data())
    }
}

impl PacketDup for Subscribe {
    fn data(&self) -> Bytes {
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        bytes.freeze()
    }

    fn dup_data(&mut self) -> Arc<Bytes> {
        Arc::new(self.data())
    }
}
impl PacketDup for Unsubscribe {
    fn data(&self) -> Bytes {
        let mut bytes = BytesMut::new();
        self.write(&mut bytes);
        bytes.freeze()
    }

    fn dup_data(&mut self) -> Arc<Bytes> {
        Arc::new(self.data())
    }
}
