use openwoot::{
    protocol::{StockIdentity, UpdateMode},
    usb::Face,
};

pub(crate) const EXAMPLE_SERIAL: &str = "A01B2401W01T00100S01H00300";

#[derive(Debug, Default)]
pub(crate) struct Recorder {
    pub(crate) update_mode_options: Vec<u8>,
}

impl UpdateMode for Recorder {
    fn enter_update_mode(&mut self, option: u8) {
        self.update_mode_options.push(option);
    }
}

#[must_use]
pub(crate) fn stock_identity(serial: Option<&'static str>) -> StockIdentity {
    StockIdentity {
        serial,
        face: Face::default(),
        layout: 0,
        mcu_id: [0; 12],
        flash_ids: [0; 10],
    }
}
