use hal::{
    flash,
    gpio::{Output, Pin},
    spi::Device,
};
use openwoot::storage::{
    self, DeviceInfo, FlashIds, LayoutVariantSource, MCU_ID_LEN, McuPage, NorLayout, Opened,
    OptionBit, Partition, SettingsRegion,
};
use openwoot_drivers::spi_nor::SpiNor;
use openwoot_image::Region;

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
pub const DEVICE_INFO: McuPage = McuPage;

const _: () = assert!(
    DEVICE_INFO_PAGE.start >= crate::IMAGE.end,
    "the device-information page lies behind the image"
);

pub type ChipSelect = Pin<'B', 12, Output>;
pub type Nor = SpiNor<Device<ChipSelect>>;
pub type Settings = storage::Settings<Nor>;

#[derive(Debug)]
pub struct Storage {
    pub settings: Option<Settings>,
    pub opened: Result<Opened, storage::Error>,
    pub device_info: Option<DeviceInfo>,
    pub layout_variant: u8,
    pub flash_ids: FlashIds,
    pub mcu_id: [u8; MCU_ID_LEN],
}

pub async fn start(mut nor: Nor, format_requested: bool) -> Storage {
    let flash_ids = match (nor.identification().await, nor.unique_id().await) {
        (Ok(identification), Ok(unique_id)) => FlashIds {
            manufacturer: identification.manufacturer,
            device: identification.device,
            unique_id,
        },
        _ => FlashIds::default(),
    };
    let device_info = flash::read(DEVICE_INFO_PAGE).and_then(|page| DEVICE_INFO.read(page).ok());
    let layout_variant = OptionBit {
        user_option: flash::user_option(),
    }
    .variant(device_info.as_ref());
    let (settings, opened) =
        match storage::Settings::open(Partition::new(nor, SETTINGS), format_requested).await {
            Ok((settings, opened)) => (Some(settings), Ok(opened)),
            Err(error) => (None, Err(error)),
        };
    Storage {
        settings,
        opened,
        device_info,
        layout_variant,
        flash_ids,
        mcu_id: flash::unique_id(),
    }
}

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
