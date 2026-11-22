#![no_std]
#![forbid(unsafe_code)]

mod is31fl3741;
mod spi_nor;

pub use is31fl3741::Is31fl3741;
pub use spi_nor::SpiNor;
