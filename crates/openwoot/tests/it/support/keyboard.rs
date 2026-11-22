use openwoot::{Keyboard, protocol::StockIdentity, usb::Face};
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

pub(crate) const EXAMPLE_SERIAL: &str = "A01B2401W01T00100S01H00300";

#[derive(Debug, Default)]
pub(crate) struct Recorder {
    pub(crate) update_mode_options: Vec<u8>,
}

impl Keyboard for Recorder {
    const DESCRIPTOR: Descriptor = Descriptor::new(
        "Test keyboard",
        UsbIds {
            vendor_id: 0x31E3,
            product_id_base: 0x1400,
        },
        Region {
            start: 0x0800_8000,
            end: 0x080F_E000,
        },
        Some(UpdateMode {
            product_id: 0x140F,
            block_size: 256,
        }),
    );
    const KEY_COLUMNS: usize = 17;
    const KEY_ROWS: usize = 6;

    fn enter_update_mode(&mut self, option: u8) {
        self.update_mode_options.push(option);
    }
}

#[must_use]
pub(crate) fn stock_identity(serial: Option<&'static str>) -> StockIdentity {
    StockIdentity {
        serial,
        face: Face::default(),
    }
}
