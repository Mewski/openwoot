use hal::{
    gpio::{Output, Pin},
    spi::Device,
};
use openwoot::storage::{self, InDeviceInfo, NorLayout, NorSector, SettingsRegion};
use openwoot_drivers::spi_nor::SpiNor;
use openwoot_image::Region;
use static_cell::ConstStaticCell;

use crate::hal;

pub const NOR_CAPACITY: u32 = 0x80_0000;
pub const DEVICE_INFO_SECTOR: Region = Region {
    start: 0x7F_F000,
    end: NOR_CAPACITY,
};
pub const NOR: NorLayout = NorLayout {
    capacity: NOR_CAPACITY,
    settings: Region {
        start: 0,
        end: 0x8_0000,
    },
    device_info: Some(DEVICE_INFO_SECTOR),
};
pub const SETTINGS: SettingsRegion = NOR.settings_region();
pub const DEVICE_INFO: NorSector = NorSector {
    offset: DEVICE_INFO_SECTOR.start,
};
pub const LAYOUT_VARIANT: InDeviceInfo = InDeviceInfo;

pub type ChipSelect = Pin<'D', 2, Output>;
pub type Nor = SpiNor<Device<ChipSelect>>;
pub type Storage = storage::Storage<'static, Nor>;

pub static SETTINGS_STORE: ConstStaticCell<storage::Settings<Nor>> =
    ConstStaticCell::new(storage::Settings::new());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_keep_clear_of_the_device_information_sector() {
        assert!(NOR.is_valid());
        assert_eq!(
            SETTINGS.region(),
            Region {
                start: 0,
                end: 128 * 4096
            }
        );
        assert_eq!(DEVICE_INFO_SECTOR.end - DEVICE_INFO_SECTOR.start, 4096);
        assert!(SETTINGS.region().end <= DEVICE_INFO_SECTOR.start);
    }
}
