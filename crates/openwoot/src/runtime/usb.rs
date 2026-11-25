use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::{
    UsbDevice,
    driver::{self, Driver},
};

use super::{Channel, Device, KeyReports, Reports};
use crate::usb::{self, Face, HostState, Identity, Negotiated, Resources};

pub struct Usb<'d, D: Driver<'d>> {
    pub device: UsbDevice<'d, Negotiated<D>>,
    pub channel: Channel<'d, Negotiated<D>>,
    pub reports: Reports<'d, Negotiated<D>>,
}

pub struct UsbParts<'d, D, M: RawMutex> {
    pub(super) driver: D,
    pub(super) negotiated_speed: fn() -> Option<driver::Speed>,
    pub(super) face: Face,
    pub(super) identity: Identity<'d>,
    pub(super) resources: &'d mut Resources<'d>,
    pub(super) host: &'d HostState,
    pub(super) key_reports: &'d KeyReports<M>,
}

pub struct UsbTasks<'d, D: Driver<'d>, M: RawMutex> {
    pub device: Device<'d, Negotiated<D>, M>,
    pub channel: Channel<'d, Negotiated<D>>,
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
            channel: Channel {
                endpoints: device.channel,
                host,
            },
            reports: Reports {
                face,
                host,
                gamepad: device.gamepad,
                keyboard: device.keyboard,
                controls: device.controls,
                key_states: device.key_states,
            },
        }
    }
}

impl<'d, D: Driver<'d>, M: RawMutex> UsbParts<'d, D, M> {
    #[must_use]
    pub fn build(self) -> UsbTasks<'d, D, M> {
        let Usb {
            device,
            channel,
            reports,
        } = Usb::new(
            self.driver,
            self.negotiated_speed,
            self.face,
            self.identity,
            self.resources,
            self.host,
        );
        UsbTasks {
            device: Device::new(device, self.key_reports),
            channel,
            reports,
        }
    }
}
