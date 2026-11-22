use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embassy_usb::{
    UsbDevice,
    driver::{self, Driver},
};

use super::{Channel, Lighting, Reports, State, Usb};
use crate::{
    lighting,
    usb::{Face, Identity, Negotiated},
};

pub struct Firmware<
    D: Driver<'static>,
    M: RawMutex + 'static,
    H: 'static,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    pub device: UsbDevice<'static, Negotiated<D>>,
    pub channel: Channel<'static, Negotiated<D>>,
    pub reports: Reports<'static, Negotiated<D>>,
    pub lighting: Lighting<'static, M, H, ROWS, COLUMNS, PAGES, LENGTH>,
}

impl<D, M, H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Firmware<D, M, H, ROWS, COLUMNS, PAGES, LENGTH>
where
    D: Driver<'static>,
    M: RawMutex + 'static,
    H: 'static,
{
    pub fn new(
        state: &'static State<M, H, ROWS, COLUMNS, PAGES, LENGTH>,
        driver: D,
        negotiated_speed: fn() -> Option<driver::Speed>,
        face: Face,
        identity: Identity<'static>,
        lighting: lighting::Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
    ) -> Self {
        let Usb {
            device,
            channel,
            reports,
        } = Usb::new(
            driver,
            negotiated_speed,
            face,
            identity,
            state.resources.take(),
            &state.host,
        );
        Self {
            device,
            channel,
            reports,
            lighting: Lighting {
                lighting: state.lighting.init(Mutex::new(lighting)),
                events: &state.key_events,
                host: &state.host,
            },
        }
    }
}
