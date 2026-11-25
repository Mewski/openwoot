use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
};
use heapless::Vec;
use openwoot_protocol::{
    configuration::MAX_KEYS,
    host_lighting::Frame,
    lightbar::{self, MAX_LEDS},
    lighting::{KeyColors, Rgb},
};

use super::{
    Lighting,
    hooks::{Lightbar, Signals},
    readback::{Readback, Subject},
};
use crate::keys::Key;

const REQUEST_CAPACITY: usize = 16;

type Mailbox<const N: usize> = Mutex<CriticalSectionRawMutex, RefCell<Vec<Rgb, N>>>;

pub struct HostControl {
    requests: Channel<CriticalSectionRawMutex, Request, REQUEST_CAPACITY>,
    keys: Mailbox<MAX_KEYS>,
    lightbar: Mailbox<MAX_LEDS>,
    readback: Readback,
    led_state: AtomicU8,
    session_ended: AtomicBool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LedState {
    pub(crate) host_control: bool,
    pub(crate) idle_off: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    Enter,
    SetKeyColor { key: Key, color: Rgb },
    SetColors,
    ResetKeyColor(Key),
    ResetColors,
    SetLightbar,
    ResetLightbar,
    Leave,
}

impl HostControl {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            requests: Channel::new(),
            keys: Mutex::new(RefCell::new(KeyColors::new())),
            lightbar: Mutex::new(RefCell::new(lightbar::Colors::new())),
            readback: Readback::new(),
            led_state: AtomicU8::new(0),
            session_ended: AtomicBool::new(false),
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
                    let mut colors = KeyColors::new();
                    load(&self.keys, &mut colors);
                    lighting.set_host_colors(&colors, now_ms);
                }
                Request::ResetKeyColor(key) => lighting.reset_host_key_color(key),
                Request::ResetColors => lighting.reset_host_colors(),
                Request::SetLightbar => {
                    let mut colors = lightbar::Colors::new();
                    load(&self.lightbar, &mut colors);
                    lighting.set_host_lightbar(&colors);
                }
                Request::ResetLightbar => lighting.reset_host_lightbar(),
                Request::Leave => lighting.leave_host_control(now_ms),
            }
        }
        self.led_state.store(
            LedState {
                host_control: lighting.host_control() || lighting.host_lightbar(),
                idle_off: lighting.is_idle_off(),
            }
            .to_bits(),
            Ordering::Release,
        );
        self.answer_readback(lighting);
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
            store(&self.keys, &filled);
        }
        self.requests.send(Request::SetColors).await;
    }

    pub(crate) async fn reset_key_color(&self, key: Key) {
        self.requests.send(Request::ResetKeyColor(key)).await;
    }

    pub(crate) async fn reset_colors(&self) {
        self.requests.send(Request::ResetColors).await;
    }

    pub(crate) async fn set_lightbar(&self, colors: &[Rgb]) {
        store(&self.lightbar, colors);
        self.requests.send(Request::SetLightbar).await;
    }

    pub(crate) async fn reset_lightbar(&self) {
        self.requests.send(Request::ResetLightbar).await;
    }

    pub(crate) async fn leave(&self) {
        self.requests.send(Request::Leave).await;
    }

    pub(crate) async fn read_frame(&self, frame: &mut Frame) {
        self.readback.read(Subject::Frame).await;
        load(&self.keys, &mut frame.keys);
        load(&self.lightbar, &mut frame.lightbar);
    }

    pub(crate) async fn read_lightbar(&self, colors: &mut lightbar::Colors) {
        self.readback.read(Subject::Lightbar).await;
        load(&self.lightbar, colors);
    }

    pub(crate) async fn led_state(&self) -> LedState {
        if !self.requests.is_empty() {
            self.readback.read(Subject::State).await;
        }
        LedState::from_bits(self.led_state.load(Ordering::Acquire))
    }

    pub(crate) fn end_session(&self) {
        self.session_ended.store(true, Ordering::Release);
    }

    #[must_use]
    pub(crate) fn take_session_end(&self) -> bool {
        self.session_ended.swap(false, Ordering::AcqRel)
    }

    fn answer_readback<
        H,
        const ROWS: usize,
        const COLUMNS: usize,
        const PAGES: usize,
        const LENGTH: usize,
    >(
        &self,
        lighting: &Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>,
    ) where
        H: Lightbar + Signals,
    {
        let Some(subject) = self.readback.requested() else {
            return;
        };
        if subject == Subject::Frame {
            self.keys.lock(|cell| {
                if let Ok(mut keys) = cell.try_borrow_mut() {
                    lighting.shown_keys(&mut keys);
                }
            });
        }
        if subject != Subject::State {
            self.lightbar.lock(|cell| {
                if let Ok(mut colors) = cell.try_borrow_mut() {
                    lighting.shown_lightbar(&mut colors);
                }
            });
        }
        self.readback.publish(subject);
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

impl LedState {
    const HOST_CONTROL: u8 = 1;
    const IDLE_OFF: u8 = 1 << 1;

    const fn to_bits(self) -> u8 {
        let host_control = if self.host_control {
            Self::HOST_CONTROL
        } else {
            0
        };
        let idle_off = if self.idle_off { Self::IDLE_OFF } else { 0 };
        host_control | idle_off
    }

    const fn from_bits(bits: u8) -> Self {
        Self {
            host_control: bits & Self::HOST_CONTROL != 0,
            idle_off: bits & Self::IDLE_OFF != 0,
        }
    }
}

fn store<const N: usize>(mailbox: &Mailbox<N>, colors: &[Rgb]) {
    mailbox.lock(|cell| {
        if let Ok(mut shared) = cell.try_borrow_mut() {
            shared.clear();
            let _ = shared.extend_from_slice(colors);
        }
    });
}

fn load<const N: usize>(mailbox: &Mailbox<N>, colors: &mut Vec<Rgb, N>) {
    mailbox.lock(|cell| {
        if let Ok(shared) = cell.try_borrow() {
            colors.clear();
            let _ = colors.extend_from_slice(&shared);
        }
    });
}
