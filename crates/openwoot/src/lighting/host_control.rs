use core::cell::RefCell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};
use openwoot_protocol::lighting::{KeyColors, Rgb};

use super::{
    Lighting,
    hooks::{Lightbar, Signals},
};
use crate::keys::Key;

const REQUEST_CAPACITY: usize = 16;

pub struct HostControl {
    requests: Channel<CriticalSectionRawMutex, Request, REQUEST_CAPACITY>,
    colors: Mutex<CriticalSectionRawMutex, RefCell<KeyColors>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Enter,
    SetKeyColor { key: Key, color: Rgb },
    SetColors,
    ResetKeyColor(Key),
    ResetColors,
    Leave,
}

impl HostControl {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requests: Channel::new(),
            colors: Mutex::new(RefCell::new(KeyColors::new())),
        }
    }

    pub async fn ready(&self) {
        self.requests.ready_to_receive().await;
    }

    pub fn apply<
        H,
        const ROWS: usize,
        const COLUMNS: usize,
        const PAGES: usize,
        const LENGTH: usize,
    >(
        &self,
        lighting: &mut Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
        now_ms: u32,
    ) where
        H: Lightbar + Signals,
    {
        while let Ok(request) = self.requests.try_receive() {
            match request {
                Request::Enter => lighting.enter_host_control(now_ms),
                Request::SetKeyColor { key, color } => {
                    lighting.set_host_key_color(key, color, now_ms);
                }
                Request::SetColors => {
                    let colors = self.colors.lock(|colors| colors.borrow().clone());
                    lighting.set_host_colors(&colors, now_ms);
                }
                Request::ResetKeyColor(key) => lighting.reset_host_key_color(key),
                Request::ResetColors => lighting.reset_host_colors(),
                Request::Leave => lighting.leave_host_control(now_ms),
            }
        }
    }

    pub(crate) async fn enter(&self) {
        self.requests.send(Request::Enter).await;
    }

    pub(crate) async fn set_key_color(&self, key: Key, color: Rgb) {
        self.requests
            .send(Request::SetKeyColor { key, color })
            .await;
    }

    pub(crate) async fn set_colors(&self, fill: impl FnOnce(&mut KeyColors)) {
        {
            let mut filled = KeyColors::new();
            fill(&mut filled);
            self.colors.lock(|colors| {
                let mut colors = colors.borrow_mut();
                colors.clear();
                let _ = colors.extend_from_slice(&filled);
            });
        }
        self.requests.send(Request::SetColors).await;
    }

    pub(crate) async fn reset_key_color(&self, key: Key) {
        self.requests.send(Request::ResetKeyColor(key)).await;
    }

    pub(crate) async fn reset_colors(&self) {
        self.requests.send(Request::ResetColors).await;
    }

    pub(crate) async fn leave(&self) {
        self.requests.send(Request::Leave).await;
    }
}

impl Default for HostControl {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for HostControl {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HostControl")
            .field("pending", &self.requests.len())
            .finish_non_exhaustive()
    }
}
