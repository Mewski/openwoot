mod configuration;
mod lighting;
mod package;
mod reports;

pub use configuration::Configuration;
use embassy_usb::{
    UsbDevice,
    driver::{self, Driver},
};
pub use lighting::{LIGHTING_POLL_MS, LightingTask};
pub use package::version;
pub use reports::Reports;

use crate::usb::{self, Face, HostState, Identity, Negotiated, Resources};

pub struct Usb<'d, D: Driver<'d>> {
    pub device: UsbDevice<'d, Negotiated<D>>,
    pub configuration: Configuration<'d, Negotiated<D>>,
    pub reports: Reports<'d, Negotiated<D>>,
}

impl<'d, D: Driver<'d>> Usb<'d, D> {
    pub fn new(
        driver: D,
        negotiated_speed: fn() -> Option<driver::Speed>,
        face: Face,
        identity: Identity<'d>,
        resources: &'d mut Resources<'d>,
        host: &'d HostState,
    ) -> Self {
        let device = usb::build(
            Negotiated::new(driver, face, negotiated_speed),
            face,
            identity,
            resources,
            host,
        );
        Self {
            device: device.usb,
            configuration: Configuration {
                endpoints: device.channel,
                host,
            },
            reports: Reports {
                face,
                host,
                configured: device.configured,
                gamepad: device.gamepad,
                keyboard: device.keyboard,
                controls: device.controls,
                key_states: device.key_states,
            },
        }
    }
}
