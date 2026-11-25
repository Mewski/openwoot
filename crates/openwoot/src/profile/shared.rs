use core::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicBool, AtomicU16, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};
use openwoot_protocol::{global, keys::Layer, lightbar, lighting::KeyColors};

use super::{Appearance, Changes};
use crate::lighting::{HostExchange, Lightbar, Lighting, ProfileState, Signals};

pub struct Shared<M: RawMutex> {
    appearance: Mutex<M, RefCell<Option<Appearance>>>,
    global: Mutex<M, Cell<Option<global::Settings>>>,
    global_lightbar: Mutex<M, Cell<lightbar::Entries>>,
    profile_state: Mutex<M, Cell<Option<ProfileState>>>,
    host_signals: HostExchange<M>,
    fast_scan: AtomicBool,
    activating: AtomicBool,
    brightness_pending: AtomicBool,
    changes: AtomicU16,
    brightness: Signal<M, u8>,
}

impl<M: RawMutex> Shared<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            appearance: Mutex::new(RefCell::new(None)),
            global: Mutex::new(Cell::new(None)),
            global_lightbar: Mutex::new(Cell::new(lightbar::Entries::new())),
            profile_state: Mutex::new(Cell::new(None)),
            host_signals: HostExchange::new(),
            fast_scan: AtomicBool::new(false),
            activating: AtomicBool::new(false),
            brightness_pending: AtomicBool::new(false),
            changes: AtomicU16::new(0),
            brightness: Signal::new(),
        }
    }

    pub fn edit<R>(&self, edit: impl FnOnce(&mut Appearance) -> R) -> R {
        self.appearance
            .lock(|cell| edit(cell.borrow_mut().get_or_insert_with(Appearance::empty)))
    }

    pub fn begin_activation(&self) {
        self.activating.store(true, Ordering::Relaxed);
    }

    pub fn activate(&self, fast_scan: bool) {
        self.activating.store(false, Ordering::Relaxed);
        self.fast_scan.store(fast_scan, Ordering::Relaxed);
        self.mark(
            Changes::LIGHTING
                .union(Changes::COLORS)
                .union(Changes::SCAN)
                .union(Changes::ACTIVATED)
                .union(Changes::LIGHTBAR),
        );
    }

    pub fn change<R>(
        &self,
        changes: Changes,
        change: impl FnOnce(&mut Appearance) -> R,
    ) -> Option<R> {
        let result = self
            .appearance
            .lock(|cell| cell.try_borrow_mut().ok()?.as_mut().map(change));
        if result.is_some() {
            self.mark(changes);
        }
        result
    }

    pub fn read<R>(&self, read: impl FnOnce(&Appearance) -> R) -> Option<R> {
        self.appearance
            .lock(|cell| cell.try_borrow().ok()?.as_ref().map(read))
    }

    pub fn set_global(&self, settings: global::Settings) {
        self.global.lock(|cell| cell.set(Some(settings)));
        self.mark(Changes::GLOBAL);
    }

    pub fn set_global_lightbar(&self, entries: &lightbar::Entries) {
        self.global_lightbar.lock(|cell| cell.set(entries.clone()));
        self.mark(Changes::LIGHTBAR);
    }

    #[must_use]
    pub fn global_lightbar(&self) -> lightbar::Entries {
        self.global_lightbar.lock(|cell| {
            let entries = cell.take();
            cell.set(entries.clone());
            entries
        })
    }

    pub fn set_profile_state(&self, state: ProfileState) {
        self.profile_state.lock(|cell| cell.set(Some(state)));
    }

    #[must_use]
    pub const fn host_signals(&self) -> &HostExchange<M> {
        &self.host_signals
    }

    pub fn set_fast_scan(&self, fast_scan: bool) {
        if self.fast_scan.swap(fast_scan, Ordering::Relaxed) != fast_scan {
            self.mark(Changes::SCAN);
        }
    }

    #[must_use]
    pub fn take_changes(&self) -> Changes {
        Changes::from_bits(self.changes.swap(0, Ordering::AcqRel))
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
        if H::PRESENT {
            self.host_signals.sync(lighting.hooks_mut());
        }
        let changes = self.take_changes();
        if changes.contains(Changes::GLOBAL)
            && let Some(global) = self.global.lock(Cell::get)
        {
            lighting.set_idle(global.idle);
            lighting.set_num_lock_inverted(global.num_lock_inverted, now_ms);
        }
        if changes.contains(Changes::SCAN) {
            lighting.set_fast_scan(self.fast_scan.load(Ordering::Relaxed));
        }
        if changes.contains(Changes::LIGHTING)
            && let Some(settings) = self.read(|appearance| appearance.settings.clone())
        {
            lighting.apply_settings(&settings, now_ms);
        }
        if changes.contains(Changes::COLORS)
            && let Some(colors) = self.read(|appearance| appearance.colors.clone())
        {
            lighting.set_base_colors(colors.layer(Layer::Base).unwrap_or(&KeyColors::new()));
            for layer in [Layer::One, Layer::Two, Layer::Three] {
                lighting.set_layer_colors(layer.number(), colors.layer(layer));
            }
        }
        if H::PRESENT
            && changes.contains(Changes::LIGHTBAR)
            && let Some(settings) = self.read(|appearance| appearance.lightbar.clone())
        {
            let global = self.global_lightbar();
            let key_brightness = lighting.brightness();
            lighting
                .hooks_mut()
                .configure(&settings, &global, key_brightness);
        }
        if let Some(state) = self.profile_state.lock(Cell::take) {
            lighting.hooks_mut().profile_changed(state);
        }
        if changes.contains(Changes::ACTIVATED) {
            lighting.show_layer(0);
        }
    }

    pub fn save_brightness(&self, level: u8) {
        if self.activating.load(Ordering::Relaxed) {
            return;
        }
        self.appearance.lock(|cell| {
            if let Ok(mut appearance) = cell.try_borrow_mut()
                && let Some(appearance) = appearance.as_mut()
            {
                appearance.settings.brightness = level;
            }
        });
        self.brightness_pending.store(true, Ordering::Release);
        self.brightness.signal(level);
    }

    pub async fn brightness_saved(&self) -> u8 {
        self.brightness.wait().await
    }

    #[must_use]
    pub fn take_brightness_save(&self) -> bool {
        self.brightness_pending.swap(false, Ordering::AcqRel)
    }

    fn mark(&self, changes: Changes) {
        self.changes.fetch_or(changes.bits(), Ordering::AcqRel);
    }
}

impl<M: RawMutex> Default for Shared<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> core::fmt::Debug for Shared<M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("fast_scan", &self.fast_scan)
            .field("changes", &self.changes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;

    use super::*;

    #[test]
    fn a_brightness_level_saved_while_a_profile_is_activated_is_dropped() {
        let shared = Shared::<NoopRawMutex>::new();
        shared.edit(|appearance| appearance.settings.brightness = 200);
        shared.begin_activation();
        shared.save_brightness(10);
        assert!(!shared.take_brightness_save());
        assert_eq!(
            shared.read(|appearance| appearance.settings.brightness),
            Some(200)
        );
        shared.activate(false);
        shared.save_brightness(20);
        assert_eq!(
            shared.read(|appearance| appearance.settings.brightness),
            Some(20)
        );
        assert!(shared.take_brightness_save());
        assert!(!shared.take_brightness_save());
    }

    #[test]
    fn a_brightness_save_stays_pending_after_its_wake_up_until_one_side_takes_it() {
        let shared = Shared::<NoopRawMutex>::new();
        shared.edit(|appearance| appearance.settings.brightness = 200);
        shared.save_brightness(30);
        assert_eq!(embassy_futures::block_on(shared.brightness_saved()), 30);
        assert!(shared.take_brightness_save());
        shared.save_brightness(40);
        assert!(shared.take_brightness_save());
        assert_eq!(embassy_futures::block_on(shared.brightness_saved()), 40);
        assert!(!shared.take_brightness_save());
    }
}
