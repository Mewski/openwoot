#![no_std]
#![forbid(unsafe_code)]

pub mod advanced;
pub mod config;
pub mod device;
mod endpoint;
mod error;
pub mod gamepad;
pub mod keys;
pub mod lightbar;
pub mod lighting;
mod message;
pub mod profile;
mod report;
pub mod stock;
pub mod switches;
mod version;

pub use endpoint::{
    ActivateProfile, CreateProfile, DeleteProfile, ENDPOINT_LIST, EnterUpdateMode, FactoryReset,
    GetAdvancedKeys, GetCapabilities, GetDeviceSettings, GetGamepadMapping, GetGamepadSettings,
    GetGlobalLightbar, GetGlobalSettings, GetHostSignals, GetKeyColors, GetKeyOverrides,
    GetKeySettings, GetKeymap, GetLayout, GetLightbar, GetLighting, GetProfileInfo,
    GetProfileRevision, GetProfiles, GetSwitchModels, GetSwitches, GetVersion, Revert, Save,
    SetAdvancedKeys, SetDeviceSettings, SetGamepadMapping, SetGamepadSettings, SetGlobalLightbar,
    SetGlobalSettings, SetHostSignals, SetKeyColors, SetKeyOverrides, SetKeySettings, SetKeymap,
    SetLayout, SetLightbar, SetLighting, SetProfileInfo, SetSwitches,
};
pub use error::Error;
pub use heapless;
pub use message::{Message, decode_message, decode_response, encode_message, encode_request};
pub use postcard_rpc::{
    Endpoint, Key,
    header::{VarHeader, VarKey, VarSeq},
    standard_icd::{ERROR_KEY, WireError},
};
pub use report::{
    Contents, FRAME_OFFSET, MAGIC, MAX_FRAME_LEN, MAX_REPORT_LEN, REPORT_SIZES, parse_report,
    seal_report,
};
pub use version::{FirmwareVersion, PROTOCOL_VERSION, Version};
