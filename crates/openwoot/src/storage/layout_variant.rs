use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::configuration::LAYOUTS;
use serde::{Deserialize, Serialize};

use super::{
    device_info::DeviceInfo,
    record::{Record, Scope},
    settings::Settings,
};

const OPTION_LAYOUT_BIT: u8 = 1 << 0;

pub trait LayoutVariantSource {
    fn variant(&self, device_info: Option<&DeviceInfo>) -> u8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InDeviceInfo;

#[derive(Clone, Copy, Debug)]
pub struct OptionBit {
    pub user_option: fn() -> u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutOverride(pub u8);

impl LayoutOverride {
    pub async fn variant_or<F: MultiwriteNorFlash>(settings: &mut Settings<F>, factory: u8) -> u8 {
        settings
            .load::<Self>(Scope::Global)
            .await
            .ok()
            .flatten()
            .map(|stored| stored.0)
            .filter(|variant| usize::from(*variant) < LAYOUTS)
            .unwrap_or(factory)
    }
}

impl Record for LayoutOverride {
    const ITEM: u8 = 0x16;
    const VERSION: u8 = 1;
}

impl LayoutVariantSource for InDeviceInfo {
    fn variant(&self, device_info: Option<&DeviceInfo>) -> u8 {
        device_info
            .and_then(|info| info.physical_layout)
            .unwrap_or_default()
    }
}

impl LayoutVariantSource for OptionBit {
    fn variant(&self, _device_info: Option<&DeviceInfo>) -> u8 {
        (self.user_option)() & OPTION_LAYOUT_BIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SerialNumber;

    #[test]
    fn the_variant_comes_from_the_record_or_the_option_byte() {
        let info = DeviceInfo {
            serial: SerialNumber::default(),
            aux: None,
            physical_layout: Some(2),
            lock_mode: None,
        };
        assert_eq!(InDeviceInfo.variant(Some(&info)), 2);
        assert_eq!(InDeviceInfo.variant(None), 0);
        let without_layout = DeviceInfo {
            physical_layout: None,
            ..info
        };
        assert_eq!(InDeviceInfo.variant(Some(&without_layout)), 0);
        assert_eq!(
            OptionBit {
                user_option: || 0xFF
            }
            .variant(Some(&info)),
            1
        );
        assert_eq!(
            OptionBit {
                user_option: || 0xFE
            }
            .variant(None),
            0
        );
    }
}
