use openwoot_protocol::{
    lightbar::{self, HostSignal, HostSignalIds},
    lighting::Rgb,
};

use super::{canvas::Canvas, indicators::LockState, input::KeyEvent, signals::ProfileState};
use crate::keys::Key;

pub trait Lightbar {
    fn needs_render(&self, since_render_ms: u16) -> bool {
        let _ = since_render_ms;
        false
    }

    fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u16) {
        let _ = (canvas, elapsed_ms);
    }

    fn request_redraw(&mut self) {}

    fn brightness(&self) -> u8 {
        0
    }

    fn apply_brightness_change(&mut self, key_brightness: u8) {
        let _ = key_brightness;
    }

    fn apply_page_brightness(&self, page: usize, linear: &[u8], output: &mut [u8], fade_level: u8) {
        let _ = (page, linear, output, fade_level);
    }

    fn owns_global_overlays(&self) -> bool {
        false
    }

    fn draw_overlay(&mut self, canvas: &mut impl Canvas, colors: &[Rgb]) {
        let _ = (canvas, colors);
    }

    fn clear_overlay(&mut self, canvas: &mut impl Canvas) {
        let _ = canvas;
    }

    fn configure(
        &mut self,
        profile: &lightbar::Settings,
        global: &lightbar::Entries,
        key_brightness: u8,
    ) {
        let _ = (profile, global, key_brightness);
    }

    fn set_fast_scan(&mut self, fast_scan: bool) {
        let _ = fast_scan;
    }

    fn set_host_control(&mut self, host_control: bool) {
        let _ = host_control;
    }

    fn take_brightness_change(&mut self) -> bool {
        false
    }
}

pub trait Signals {
    fn brightness_changed(&mut self, percent: f32) {
        let _ = percent;
    }

    fn profile_changed(&mut self, profile: ProfileState) {
        let _ = profile;
    }

    fn locks_changed(&mut self, locks: LockState) {
        let _ = locks;
    }

    fn layer_changed(&mut self, layer: u8) {
        let _ = layer;
    }

    fn key_event(&mut self, key: Key, event: KeyEvent) {
        let _ = (key, event);
    }

    fn travel_changed(&mut self, deepest: u16) {
        let _ = deepest;
    }

    fn tick(&mut self, now_ms: u32) {
        let _ = now_ms;
    }

    fn activate_host_signals(&mut self) {}

    fn set_host_signals(&mut self, values: &[HostSignal]) {
        let _ = values;
    }

    fn host_signal_ids(&self) -> HostSignalIds {
        HostSignalIds::new()
    }
}

impl Lightbar for () {}

impl Signals for () {}
