#![allow(dead_code, unused)]

extern crate core;

pub mod v3_1_1;
mod tasks;
pub mod utils;

use tokio::sync::mpsc::{channel, Receiver, Sender};
use crate::utils::Endpoint;


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


