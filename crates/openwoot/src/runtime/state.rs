use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use static_cell::{ConstStaticCell, StaticCell};

use super::{KeyNotices, KeyReports};
use crate::{
    keys,
    lighting::{self, KeyEvents},
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
    pub(super) host: HostState,
    pub(super) key_events: KeyEvents,
    pub(super) key_notices: KeyNotices,
    pub(super) key_reports: KeyReports<M>,
    pub(super) keys: keys::Shared<M>,
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
            host: HostState::new(),
            key_events: KeyEvents::new(),
            key_notices: KeyNotices::new(),
            key_reports: KeyReports::new(),
            keys: keys::Shared::new(),
            lighting: StaticCell::new(),
        }
    }

    #[must_use]
    pub const fn keys(&self) -> &keys::Shared<M> {
        &self.keys
    }

    #[must_use]
    pub const fn key_reports(&self) -> &KeyReports<M> {
        &self.key_reports
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
