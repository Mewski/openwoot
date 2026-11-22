use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embassy_usb::driver;
use static_cell::{ConstStaticCell, StaticCell};

use super::{KeyNotices, KeyReports};
use crate::{
    diagnostics::{Diagnostics, Hardware, Readout},
    keys,
    lighting::{self, HostControl, KeyEvents},
    profile,
    scan::SensorReadout,
    usb::{HostState, Resources},
};

pub struct State<
    M: RawMutex,
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    pub(super) resources: ConstStaticCell<Resources<'static>>,
    pub(super) diagnostics: Diagnostics,
    pub(super) host: HostState,
    pub(super) host_control: HostControl,
    pub(super) sensors: SensorReadout,
    pub(super) key_events: KeyEvents,
    pub(super) key_notices: KeyNotices,
    pub(super) key_reports: KeyReports<M>,
    pub(super) keys: keys::Shared<M>,
    pub(super) profile: profile::Shared<M>,
    pub(super) lighting: StaticCell<Mutex<M, lighting::Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>>>,
}

impl<
    M: RawMutex,
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> State<M, H, ROWS, COLUMNS, PAGES, LENGTH>
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            resources: ConstStaticCell::new(Resources::new()),
            diagnostics: Diagnostics::new(),
            host: HostState::new(),
            host_control: HostControl::new(),
            sensors: SensorReadout::new(),
            key_events: KeyEvents::new(),
            key_notices: KeyNotices::new(),
            key_reports: KeyReports::new(),
            keys: keys::Shared::new(),
            profile: profile::Shared::new(),
            lighting: StaticCell::new(),
        }
    }

    #[must_use]
    pub const fn keys(&self) -> &keys::Shared<M> {
        &self.keys
    }

    #[must_use]
    pub const fn profile(&self) -> &profile::Shared<M> {
        &self.profile
    }

    #[must_use]
    pub const fn key_reports(&self) -> &KeyReports<M> {
        &self.key_reports
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    #[must_use]
    pub const fn host(&self) -> &HostState {
        &self.host
    }

    #[must_use]
    pub const fn host_control(&self) -> &HostControl {
        &self.host_control
    }

    #[must_use]
    pub const fn sensors(&self) -> &SensorReadout {
        &self.sensors
    }

    #[must_use]
    pub const fn readout(
        &self,
        negotiated_speed: fn() -> Option<driver::Speed>,
        hardware: fn() -> Hardware,
    ) -> Readout<'_> {
        Readout {
            diagnostics: &self.diagnostics,
            host: &self.host,
            negotiated_speed,
            hardware,
        }
    }
}

impl<
    M: RawMutex,
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> Default for State<M, H, ROWS, COLUMNS, PAGES, LENGTH>
{
    fn default() -> Self {
        Self::new()
    }
}
