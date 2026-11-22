use embassy_usb::driver;
use openwoot_protocol::diagnostics::{Report, Speed, Usb};

use super::{Diagnostics, Hardware};
use crate::usb::HostState;

#[derive(Debug, Clone, Copy)]
pub struct Readout<'d> {
    pub diagnostics: &'d Diagnostics,
    pub host: &'d HostState,
    pub negotiated_speed: fn() -> Option<driver::Speed>,
    pub hardware: fn() -> Hardware,
}

impl Readout<'_> {
    #[must_use]
    pub fn report(&self) -> Report {
        let usb = Usb {
            configured: self.host.is_configured(),
            suspended: self.host.is_suspended(),
            speed: (self.negotiated_speed)().map(|speed| match speed {
                driver::Speed::Low => Speed::Low,
                driver::Speed::Full => Speed::Full,
                driver::Speed::High => Speed::High,
            }),
            resets: self.host.resets(),
        };
        self.diagnostics.report(usb, (self.hardware)())
    }
}
