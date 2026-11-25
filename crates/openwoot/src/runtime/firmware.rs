use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::driver::{self, Driver};

use super::{KeyReports, KeyTask, Lighting, State, UsbParts};
use crate::{
    lighting,
    usb::{Face, Identity},
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
    pub usb: UsbParts<'static, D, M>,
    pub key_reports: &'static KeyReports<M>,
    pub keys: KeyTask<'static, M>,
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
        Self {
            usb: UsbParts {
                driver,
                negotiated_speed,
                face,
                identity,
                resources: state.resources.take(),
                host: &state.host,
                key_reports: &state.key_reports,
            },
            key_reports: &state.key_reports,
            keys: KeyTask {
                shared: &state.keys,
                reports: &state.key_reports,
                events: &state.key_events,
                notices: &state.key_notices,
                host: &state.host,
                sensors: &state.sensors,
                face,
                diagnostics: &state.diagnostics,
                led_window: &state.led_window,
            },
            lighting: Lighting {
                lighting: state.lighting.init(lighting),
                events: &state.key_events,
                notices: &state.key_notices,
                host: &state.host,
                host_control: &state.host_control,
                diagnostics: &state.diagnostics,
                profile: &state.profile,
                led_window: &state.led_window,
            },
        }
    }
}
