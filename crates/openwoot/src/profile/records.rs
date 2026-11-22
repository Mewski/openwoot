use openwoot_protocol::{device, global, lightbar, lighting, profile::Info};
use serde::{Deserialize, Serialize};

use super::Colors;
use crate::storage::Record;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count(pub u8);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision(pub u32);

impl Record for lighting::Settings {
    const ITEM: u8 = 0x20;
    const VERSION: u8 = 1;
}

impl Record for Colors {
    const ITEM: u8 = 0x21;
    const VERSION: u8 = 1;
}

impl Record for Info {
    const ITEM: u8 = 0x22;
    const VERSION: u8 = 1;
}

impl Record for Revision {
    const ITEM: u8 = 0x23;
    const VERSION: u8 = 1;
}

impl Record for global::Settings {
    const ITEM: u8 = 0x11;
    const VERSION: u8 = 1;
}

impl Record for device::Settings {
    const ITEM: u8 = 0x12;
    const VERSION: u8 = 1;
}

impl Record for Count {
    const ITEM: u8 = 0x13;
    const VERSION: u8 = 1;
}

impl Record for lightbar::Settings {
    const ITEM: u8 = 0x24;
    const VERSION: u8 = 1;
}

impl Record for lightbar::Entries {
    const ITEM: u8 = 0x14;
    const VERSION: u8 = 1;
}
