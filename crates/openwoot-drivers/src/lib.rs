#![no_std]
#![forbid(unsafe_code)]

mod is31fl3741;
mod spi_nor;

pub use is31fl3741::{Is31fl3741, LedConfiguration, LedError, LedPage, PwmBank};
pub use spi_nor::{
    DEVICE_64_MBIT, GIGADEVICE, NOR_PAGE_SIZE, NOR_SECTOR_SIZE, NOR_UNIQUE_ID_LEN, NorError,
    NorIdentification, SpiNor, WINBOND,
};
