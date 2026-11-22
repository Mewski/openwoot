mod device_info;
mod format;
mod layout_variant;
mod nor_identity;
mod nor_layout;
mod partition;
mod record;
mod settings;
mod store;

pub use device_info::{
    AuxRecord, DeviceInfo, DeviceInfoError, DeviceInfoSource, LEGACY_SERIAL_LEN, McuPage,
    NorSector, SerialMessage, SerialNumber, SerialText,
};
pub use format::{FormatReason, Opened};
pub use layout_variant::{InDeviceInfo, LayoutVariantSource, OptionBit};
pub use nor_identity::NorIdentity;
pub use nor_layout::{NorLayout, SECTOR_SIZE, SettingsRegion};
pub use partition::{Partition, PartitionError};
pub use record::{Key, Record, Scope, Stored};
pub use settings::{Error, Settings};
pub use store::Storage;
