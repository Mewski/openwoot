#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

mod descriptor;
mod linker_script;
mod validation;

pub use descriptor::{Descriptor, DescriptorError, Region, UpdateMode, UsbIds};
pub use linker_script::{LinkerScriptError, MemoryLayout};
pub use validation::{Error, Image};
