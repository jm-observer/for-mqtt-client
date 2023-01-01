#![allow(dead_code, unused_mut, unused_imports, unused_variables)]
extern crate core;

mod tasks;
pub mod utils;
pub mod v3_1_1;

#[derive(Clone)]
pub enum Transport {
    Tcp,
}

/// Quality of service
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum QoS {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}
