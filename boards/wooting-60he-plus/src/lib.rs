#![no_std]
#![forbid(unsafe_code)]

pub use cortex_m_rt::entry;
use openwoot::Keyboard;
pub use openwoot_gd32f3 as hal;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

include!(concat!(env!("OUT_DIR"), "/memory_layout.rs"));

pub const NAME: &str = "Wooting 60HE+";
pub const USB_IDS: UsbIds = UsbIds {
    vendor_id: 0x31E3,
    product_id_base: 0x1320,
};
pub const UPDATE_MODE: Option<UpdateMode> = None;
pub const BOOTLOADER: Region = Region {
    start: 0x0800_0000,
    end: 0x0800_7000,
};
pub const SERIAL_PAGE: Region = Region {
    start: 0x080F_D000,
    end: 0x080F_E000,
};
pub const KEY_ROWS: usize = 6;
pub const KEY_COLUMNS: usize = 14;
pub const DESCRIPTOR: Descriptor = Descriptor::new(NAME, USB_IDS, IMAGE, UPDATE_MODE);

#[derive(Debug, Default)]
pub struct Board;

impl Board {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Keyboard for Board {
    const DESCRIPTOR: Descriptor = DESCRIPTOR;
    const KEY_COLUMNS: usize = KEY_COLUMNS;
    const KEY_ROWS: usize = KEY_ROWS;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_keeps_clear_of_the_bootloader_and_the_serial_page() {
        assert_eq!(IMAGE.start, BOOTLOADER.end);
        assert_eq!(IMAGE.end, SERIAL_PAGE.start);
    }
}
