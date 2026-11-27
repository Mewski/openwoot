#![no_std]
#![forbid(unsafe_code)]
#![allow(
    long_running_const_eval,
    reason = "postcard-rpc checks every schema of the endpoint table for unique keys at compile time"
)]

extern crate self as openwoot_protocol;

pub mod advanced;
pub mod apps;
pub mod configuration;
pub mod device;
pub mod diagnostics;
pub mod endpoints;
mod error;
pub mod gamepad;
pub mod geometry;
pub mod global;
pub mod host_lighting;
pub mod keys;
pub mod lightbar;
pub mod lighting;
pub mod macros;
mod message;
pub mod profile;
pub mod report;
pub mod sensors;
pub mod stock;
pub mod switches;
mod version;
pub mod wire;

pub use error::Error;
pub use heapless;
pub use message::{
    Message, decode_message, decode_response, encode_frame, encode_message, encode_request,
};
pub use postcard_rpc::{
    Endpoint, Key,
    header::{VarHeader, VarKey, VarSeq},
    standard_icd::{ERROR_KEY, WireError},
};
pub use version::{FirmwareVersion, PROTOCOL_VERSION, Version};
