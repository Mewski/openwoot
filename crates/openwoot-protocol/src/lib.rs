#![no_std]
#![forbid(unsafe_code)]

pub mod advanced;
pub mod configuration;
pub mod device;
pub mod diagnostics;
pub mod endpoints;
mod error;
pub mod gamepad;
pub mod global;
pub mod keys;
pub mod lightbar;
pub mod lighting;
mod message;
pub mod profile;
pub mod report;
pub mod stock;
pub mod switches;
mod version;

pub use error::Error;
pub use heapless;
pub use message::{Message, decode_message, decode_response, encode_message, encode_request};
pub use postcard_rpc::{
    Endpoint, Key,
    header::{VarHeader, VarKey, VarSeq},
    standard_icd::{ERROR_KEY, WireError},
};
pub use version::{FirmwareVersion, PROTOCOL_VERSION, Version};
