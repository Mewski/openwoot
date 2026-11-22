use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use static_cell::{ConstStaticCell, StaticCell};

use crate::{
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
            lighting: StaticCell::new(),
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
