use std::sync::atomic::{AtomicU32, Ordering};

static ID: AtomicU32 = AtomicU32::new(0);
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Id(pub(crate) u32);

impl Default for Id {
    fn default() -> Self {
        Self(ID.fetch_add(1, Ordering::Relaxed))
    }
}
impl Id {
    pub fn id() -> u32 {
        Self::default().0
    }
}

static SUBSCRIBE_ID: AtomicU32 = AtomicU32::new(1);
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubscribeId(pub(crate) u32);

impl Default for SubscribeId {
    fn default() -> Self {
        let mut id = SUBSCRIBE_ID.fetch_add(1, Ordering::Relaxed);
        id = if id == 0 || id > 268_435_455 {
            SUBSCRIBE_ID.store(1, Ordering::Relaxed);
            SUBSCRIBE_ID.load(Ordering::Relaxed)
        } else {
            id
        };
        Self(id)
    }
}
impl SubscribeId {
    pub fn id() -> u32 {
        Self::default().0
    }
}
