#![no_std]
#![forbid(unsafe_code)]

mod command;
mod framing;

pub use command::{FirmwareVersion, PROTOCOL_VERSION, Request, Response};
pub use framing::{MAGIC, MAX_FRAME_LEN};
