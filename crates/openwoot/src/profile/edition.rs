use super::Factory;
use crate::{analog::SwitchType, storage::DeviceInfo};

#[derive(Debug)]
pub struct Edition<const ROWS: usize, const COLUMNS: usize> {
    pub codes: &'static [u8],
    pub product: &'static str,
    pub factory: Factory<ROWS, COLUMNS>,
    pub switch_type: SwitchType,
}

#[derive(Clone, Copy, Debug)]
pub struct Unit<const ROWS: usize, const COLUMNS: usize> {
    pub product: &'static str,
    pub factory: &'static Factory<ROWS, COLUMNS>,
    pub switch_type: SwitchType,
    pub edition: Option<u8>,
}

impl<const ROWS: usize, const COLUMNS: usize> Edition<ROWS, COLUMNS> {
    #[must_use]
    pub fn code_of(&self, device_info: Option<&DeviceInfo>) -> Option<u8> {
        device_info
            .and_then(|info| info.serial.edition)
            .filter(|code| self.codes.contains(code))
    }

    #[must_use]
    pub fn unit(
        &'static self,
        regular: Unit<ROWS, COLUMNS>,
        device_info: Option<&DeviceInfo>,
    ) -> Unit<ROWS, COLUMNS> {
        self.code_of(device_info).map_or(regular, |code| Unit {
            product: self.product,
            factory: &self.factory,
            switch_type: self.switch_type,
            edition: Some(code),
        })
    }
}

impl<const ROWS: usize, const COLUMNS: usize> Unit<ROWS, COLUMNS> {
    #[must_use]
    pub const fn regular(product: &'static str, factory: &'static Factory<ROWS, COLUMNS>) -> Self {
        Self {
            product,
            factory,
            switch_type: SwitchType::DEFAULT,
            edition: None,
        }
    }
}
