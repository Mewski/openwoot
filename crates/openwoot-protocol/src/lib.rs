#![no_std]
#![forbid(unsafe_code)]

mod endpoint;
mod error;
mod message;
mod report;
pub mod stock;
mod version;

pub use endpoint::{ENDPOINT_LIST, EnterUpdateMode, GetVersion};
pub use error::Error;
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
