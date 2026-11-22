use openwoot::{
    protocol::{Configuration, Restart, StockIdentity, UpdateMode},
    usb::Face,
};
use openwoot_protocol::{
    configuration::{Capabilities, Rejection},
    heapless::Vec as Bounded,
};

use crate::lighting::{COLUMNS, ROWS};

pub(crate) const EXAMPLE_SERIAL: &str = "A01B2401W01T00100S01H00300";

#[derive(Debug, Default)]
pub(crate) struct Recorder {
    pub(crate) update_mode_options: Vec<u8>,
    pub(crate) restart_options: Vec<u8>,
}

impl UpdateMode for Recorder {
    fn enter_update_mode(&mut self, option: u8) {
        self.update_mode_options.push(option);
    }
}

impl Restart for Recorder {
    fn restart(&mut self, option: u8) {
        self.restart_options.push(option);
    }
}

#[derive(Debug, Default)]
pub(crate) struct KeyMatrix;

impl Configuration for KeyMatrix {
    fn capabilities(&self) -> Result<Capabilities, Rejection> {
        Ok(Capabilities {
            key_rows: u8::try_from(ROWS).unwrap_or(u8::MAX),
            key_columns: u8::try_from(COLUMNS).unwrap_or(u8::MAX),
            profiles: 1,
            layouts: Bounded::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 1,
            edition: None,
        })
    }
}

#[must_use]
pub(crate) fn stock_identity(serial: Option<&'static str>) -> StockIdentity {
    StockIdentity {
        serial,
        face: Face::default(),
        layout: 0,
        mcu_id: [0; 12],
        nor_identity: None,
    }
}
