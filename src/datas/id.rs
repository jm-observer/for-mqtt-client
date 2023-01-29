use std::sync::atomic::{AtomicU32, Ordering};

static ID: AtomicU32 = AtomicU32::new(0);
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Id(pub(crate) u32);

impl Default for Id {
    fn default() -> Self {
        Self(ID.fetch_add(1, Ordering::Relaxed))
    }
}
