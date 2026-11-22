mod device_info;
mod hardware_ids;
mod layout_variant;
mod partition;
mod record;
mod settings;

pub use device_info::{
    AUX_CAPACITY, AuxRecord, DEVICE_INFO_LEN, DeviceInfo, DeviceInfoError, McuPage, NorSector,
    SERIAL_TEXT_CAPACITY, SerialNumber, SerialText, Unmarked,
};
pub use hardware_ids::{FLASH_IDS_LEN, FLASH_UNIQUE_ID_LEN, FlashIds, MCU_ID_LEN};
pub use layout_variant::{InDeviceInfo, LayoutVariantSource, OptionBit};
pub use partition::{NorLayout, Partition, PartitionError, SECTOR_SIZE, SettingsRegion};
pub use record::{Key, Record, Scope, Stored};
pub use settings::{Error, FormatReason, MAX_KEYS, MAX_PAGES, MAX_VALUE_LEN, Opened, Settings};
