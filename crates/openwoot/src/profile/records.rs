use openwoot_protocol::{device, global, lightbar, lighting, profile::Info, switches};
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

impl Record for switches::Settings {
    const ITEM: u8 = 0x15;
    const VERSION: u8 = 1;
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{advanced, gamepad, keys};

    use super::*;
    use crate::{
        keys::{ActiveProfile, Keymaps},
        storage::LayoutOverride,
    };

    const FORMAT_ITEM: u8 = 0;

    fn assert_unique(items: &[u8]) {
        for (index, item) in items.iter().enumerate() {
            assert!(!items[..index].contains(item), "item {item:#04x} twice");
        }
    }

    #[test]
    fn every_record_of_a_scope_has_its_own_item() {
        assert_unique(&[
            <keys::Settings as Record>::ITEM,
            <Keymaps as Record>::ITEM,
            <keys::Overrides as Record>::ITEM,
            <advanced::Keys as Record>::ITEM,
            <gamepad::Mapping as Record>::ITEM,
            <gamepad::Settings as Record>::ITEM,
            <lighting::Settings as Record>::ITEM,
            <Colors as Record>::ITEM,
            <Info as Record>::ITEM,
            <Revision as Record>::ITEM,
            <lightbar::Settings as Record>::ITEM,
        ]);
        assert_unique(&[
            FORMAT_ITEM,
            <ActiveProfile as Record>::ITEM,
            <global::Settings as Record>::ITEM,
            <device::Settings as Record>::ITEM,
            <Count as Record>::ITEM,
            <lightbar::Entries as Record>::ITEM,
            <switches::Settings as Record>::ITEM,
            <LayoutOverride as Record>::ITEM,
        ]);
    }
}
