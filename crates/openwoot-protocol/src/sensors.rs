use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::configuration::MAX_KEYS;

pub type Values = Vec<Option<u16>, MAX_KEYS>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Reading {
    Travel,
    Value,
    RestLevel,
    SingleRestLevel,
    ParallelRestLevel,
    BottomLevel,
    AboveRest,
    ScanOrder,
}
