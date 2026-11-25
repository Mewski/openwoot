use hal::{
    flash,
    gpio::{Output, Pin},
    spi::Device,
};
use openwoot::storage::{self, McuPage, NorLayout, OptionBit, SettingsRegion};
use openwoot_drivers::spi_nor::SpiNor;
use openwoot_image::Region;
use static_cell::StaticCell;

use crate::hal;

pub const NOR_CAPACITY: u32 = 0x80_0000;
pub const NOR: NorLayout = NorLayout {
    capacity: NOR_CAPACITY,
    settings: Region {
        start: 0,
        end: 0x8_0000,
    },
    device_info: None,
};
pub const SETTINGS: SettingsRegion = NOR.settings_region();
pub const DEVICE_INFO_PAGE: Region = Region {
    start: 0x080F_F000,
    end: 0x0810_0000,
};
pub const DEVICE_INFO: McuPage = McuPage {
    region: DEVICE_INFO_PAGE,
    read: flash::read,
};
pub const LAYOUT_VARIANT: OptionBit = OptionBit {
    user_option: flash::user_option,
};

const _: () = assert!(
    DEVICE_INFO_PAGE.start >= crate::IMAGE.end,
    "the device-information page lies behind the image"
);

pub type ChipSelect = Pin<'B', 12, Output>;
pub type Nor = SpiNor<Device<ChipSelect>>;
pub type Storage = storage::Storage<'static, Nor>;

pub static SETTINGS_STORE: StaticCell<storage::Settings<Nor>> = StaticCell::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_are_the_first_128_sectors_and_the_device_information_one_page() {
        assert!(NOR.is_valid());
        assert_eq!(
            SETTINGS.region(),
            Region {
                start: 0,
                end: 128 * 4096
            }
        );
        assert_eq!(DEVICE_INFO_PAGE.end - DEVICE_INFO_PAGE.start, 4096);
    }
}
