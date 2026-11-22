mod host;
mod render;

use super::{
    brightness::Brightness,
    canvas::Painter,
    color::opaque,
    config::{Idle, KeyColors, Rgb, Settings},
    dirty_pages::DirtyPages,
    effects::EffectStack,
    frame::Frame,
    hooks::{Lightbar, Signals},
    idle,
    indicators::{Indicator, LockIndicators, LockState},
    input::KeyEvent,
    key_colors::{LayerColors, paint},
    leds::{Geometry, LedMap, Variant},
    overlay::{CLASS_GLOBAL, FADE_FULL, Overlay, OverlayId, Overlays, Span},
};
use crate::keys::Key;

pub const FRAME_PERIOD_MS: u32 = 16;

const DRIVER_WAKE_FADE_MS: u16 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputChange {
    Wake,
    ShutDown,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent mode switches of the lighting state, as in the reference firmware"
)]
pub struct Lighting<
    H,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    map: &'static LedMap<PAGES>,
    variant: &'static Variant<ROWS, COLUMNS>,
    geometry: Geometry,
    hooks: H,
    frame: Frame<PAGES, LENGTH>,
    dirty: DirtyPages<PAGES>,
    effects: EffectStack<ROWS, COLUMNS>,
    overlays: Overlays,
    colors: LayerColors<ROWS, COLUMNS>,
    brightness: Brightness,
    fade_level: u8,
    indicators: LockIndicators,
    idle: idle::Timer,
    host_control: bool,
    fast_scan: bool,
    output_enabled: bool,
    effect_stamp_ms: u32,
    redraw_requested: bool,
    force_render: bool,
}

impl<H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>
where
    H: Lightbar + Signals,
{
    #[must_use]
    pub const fn new(
        map: &'static LedMap<PAGES>,
        variant: &'static Variant<ROWS, COLUMNS>,
        geometry: Geometry,
        hooks: H,
        now_ms: u32,
    ) -> Self {
        Self {
            map,
            variant,
            geometry,
            hooks,
            frame: Frame::new(),
            dirty: DirtyPages::new(),
            effects: EffectStack::new(),
            overlays: Overlays::new(),
            colors: LayerColors::new(),
            brightness: Brightness::new(),
            fade_level: FADE_FULL,
            indicators: LockIndicators::new(),
            idle: idle::Timer::new(now_ms),
            host_control: false,
            fast_scan: false,
            output_enabled: false,
            effect_stamp_ms: now_ms,
            redraw_requested: false,
            force_render: false,
        }
    }

    #[must_use]
    pub const fn hooks(&self) -> &H {
        &self.hooks
    }

    pub const fn hooks_mut(&mut self) -> &mut H {
        &mut self.hooks
    }

    #[must_use]
    pub const fn frame(&self) -> &Frame<PAGES, LENGTH> {
        &self.frame
    }

    #[must_use]
    pub const fn brightness(&self) -> u8 {
        self.brightness.level()
    }

    #[must_use]
    pub const fn fade_level(&self) -> u8 {
        self.fade_level
    }

    #[must_use]
    pub const fn is_idle_off(&self) -> bool {
        self.idle.is_off()
    }

    #[must_use]
    pub const fn host_control(&self) -> bool {
        self.host_control
    }

    #[must_use]
    pub const fn output_enabled(&self) -> bool {
        self.output_enabled
    }

    #[must_use]
    pub fn key_color(&self, key: Key) -> Option<Rgb> {
        let led = self.variant.led(key.row(), key.column())?;
        self.map
            .position(led)
            .and_then(|position| self.frame.color(position))
    }

    pub fn apply_settings(&mut self, settings: &Settings, now_ms: u32) {
        self.brightness.set(settings.brightness);
        self.indicators.set_colors(settings.indicators);
        self.effects.apply(&settings.effects);
        self.paint_base_colors(false);
        self.refresh_lock_indicators(now_ms);
    }

    pub fn set_base_colors(&mut self, colors: &KeyColors) {
        self.colors.set_base(colors);
        self.paint_base_colors(false);
    }

    pub fn set_layer_colors(&mut self, layer: u8, colors: Option<&KeyColors>) {
        self.colors.set_layer(layer, colors);
    }

    pub const fn show_layer(&mut self, layer: u8) {
        self.colors.request(layer);
    }

    pub const fn set_idle(&mut self, idle: Option<Idle>) {
        self.idle.set_config(idle);
    }

    pub fn set_num_lock_inverted(&mut self, inverted: bool, now_ms: u32) {
        self.indicators.set_num_lock_inverted(inverted);
        self.refresh_lock_indicators(now_ms);
    }

    pub fn set_fast_scan(&mut self, fast_scan: bool) {
        if self.fast_scan != fast_scan {
            self.fast_scan = fast_scan;
            self.redraw_requested = true;
            self.force_render = true;
            self.paint_base_colors(false);
            self.hooks.set_fast_scan(fast_scan);
        }
    }

    #[must_use]
    pub const fn layer_lock_color(&self) -> Rgb {
        opaque(self.indicators.colors().layer_lock)
    }

    pub fn set_indicator_key(&mut self, indicator: Indicator, key: Option<Key>, now_ms: u32) {
        self.indicators.set_key(indicator, key);
        self.refresh_lock_indicators(now_ms);
    }

    pub fn update_lock_indicators(&mut self, locks: LockState, now_ms: u32) {
        self.indicators.set_locks(locks);
        self.refresh_lock_indicators(now_ms);
    }

    pub fn start_overlay(&mut self, id: OverlayId, span: Span, overlay: Overlay, now_ms: u32) {
        let lightbar_class = if self.hooks.owns_global_overlays() {
            CLASS_GLOBAL
        } else {
            0
        };
        self.overlays
            .start(id, span, overlay, now_ms, lightbar_class);
    }

    pub fn stop_overlay(&mut self, id: OverlayId) {
        self.overlays.stop(id);
    }

    #[must_use]
    pub fn overlay_running(&self, id: OverlayId) -> bool {
        self.overlays.is_running(id)
    }

    pub const fn set_output(&mut self, enabled: bool) -> Option<OutputChange> {
        if enabled == self.output_enabled {
            return None;
        }
        self.output_enabled = enabled;
        Some(if enabled {
            OutputChange::Wake
        } else {
            OutputChange::ShutDown
        })
    }

    pub fn wake(&mut self, now_ms: u32) {
        self.effect_stamp_ms = now_ms;
        if !self.host_control {
            self.start_overlay(
                OverlayId::WakeFade,
                Span::Milliseconds(DRIVER_WAKE_FADE_MS),
                Overlay::fade(0, FADE_FULL, DRIVER_WAKE_FADE_MS),
                now_ms,
            );
        }
    }

    pub fn key_event(&mut self, key: Key, event: KeyEvent, now_ms: u32) {
        if !self.host_control {
            self.effects.event(key, event, now_ms, self.variant);
        }
    }

    pub fn reset_idle_timer(&mut self, now_ms: u32) {
        if let Some(fade) = self.idle.reset()
            && !self.host_control
        {
            self.start_overlay(OverlayId::IdleFade, fade.span, fade.overlay, now_ms);
        }
    }

    pub fn step_brightness(&mut self, increase: bool, now_ms: u32) -> u8 {
        let level = self.brightness.step(increase, self.fast_scan, now_ms);
        self.apply_brightness_change();
        self.hooks.brightness_changed(self.brightness.percent());
        level
    }

    pub fn take_brightness_save(&mut self, now_ms: u32) -> Option<u8> {
        self.brightness.take_save(now_ms)
    }

    pub fn paint_base_colors(&mut self, force: bool) {
        self.redraw_requested = true;
        self.hooks.request_redraw();
        let covered = self.host_control || self.effects_shown() || self.colors.shown_layer() != 0;
        if force || !covered {
            let colors = *self.colors.base();
            paint(&mut self.main_canvas(), &colors);
        }
        self.apply_brightness_change();
    }

    fn apply_brightness_change(&mut self) {
        if self.brightness.apply() && self.fade_level != 0 {
            self.dirty.push_all();
        }
        self.hooks.apply_brightness_change(self.brightness.level());
    }

    fn main_canvas(&mut self) -> Painter<'_, ROWS, COLUMNS, PAGES, LENGTH> {
        Painter::new(
            &mut self.frame,
            self.map,
            self.variant,
            Some(&mut self.dirty),
        )
    }

    fn effects_shown(&self) -> bool {
        !self.fast_scan && !self.effects.is_empty()
    }

    fn effects_enabled(&self) -> bool {
        self.effects_shown() && self.colors.shown_layer() == 0 && !self.host_control
    }

    fn refresh_lock_indicators(&mut self, now_ms: u32) {
        for (id, overlay) in self.indicators.overlays(self.host_control) {
            match overlay {
                None => self.overlays.stop(id),
                Some(overlay) if !self.host_control => {
                    self.start_overlay(id, Span::Forever, overlay, now_ms);
                }
                Some(_) => {}
            }
        }
    }
}
