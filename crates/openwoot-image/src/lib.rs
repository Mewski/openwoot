#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

mod descriptor;
mod hex;
mod image;
mod linker_script;
mod region;

pub use descriptor::{Descriptor, DescriptorError, UpdateMode, UsbIds};
pub use hex::Hex;
pub use image::{Error, Image};
pub use linker_script::{LinkerScriptError, MemoryLayout};
pub use region::Region;
