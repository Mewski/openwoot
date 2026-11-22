use super::{
    color::{BLACK, opaque},
    config::{Color, Effect, Idle, Indicators, KeyColors, Rgb, Settings},
    effects::{Layers, low_16},
    frame::{Canvas, DirtyPages, Frame, PageWrite, Painter},
    gamma::output_scale,
    hooks::{Lightbar, Signals},
    input::{KeyEvent, KeyTravel},
    leds::{Geometry, Key, Led, LedMap, Variant},
    overlay::{CLASS_ALL, CLASS_GLOBAL, Overlay, OverlayId, Overlays, Pass, Span},
};

pub const FRAME_PERIOD_MS: u32 = 16;
pub const LAYER_COLOR_MAPS: usize = 3;
pub const BRIGHTNESS_STEP: u8 = 5;
pub const BRIGHTNESS_MAX_FAST_SCAN: u8 = 179;
pub const BRIGHTNESS_SAVE_DELAY_MS: u32 = 2000;
pub const FADE_FULL: u8 = u8::MAX;
pub const FADE_DIMMED: u8 = 127;

const IDLE_DIM_FADE_MS: u16 = 2000;
const IDLE_OFF_FADE_MS: u16 = 3000;
const IDLE_WAKE_FADE_MS: u16 = 500;
const DRIVER_WAKE_FADE_MS: u16 = 250;
const MILLISECONDS_PER_MINUTE: u32 = 60_000;

pub type KeyColorMap<const ROWS: usize, const COLUMNS: usize> = [[Rgb; COLUMNS]; ROWS];

#[expect(
    clippy::struct_excessive_bools,
    reason = "the four lock states are independent host and layer states"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LockState {
    pub caps_lock: bool,
    pub scroll_lock: bool,
    pub num_lock: bool,
    pub layer_locked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Indicator {
    CapsLock,
    ScrollLock,
    NumLock,
    LayerLock,
}

impl Indicator {
    const ALL: [Self; 4] = [
        Self::LayerLock,
        Self::CapsLock,
        Self::ScrollLock,
        Self::NumLock,
    ];

    const fn overlay(self) -> OverlayId {
        match self {
            Self::CapsLock => OverlayId::CapsLock,
            Self::ScrollLock => OverlayId::ScrollLock,
            Self::NumLock => OverlayId::NumLock,
            Self::LayerLock => OverlayId::LayerLock,
        }
    }

    const fn slot(self) -> usize {
        match self {
            Self::CapsLock => 0,
            Self::ScrollLock => 1,
            Self::NumLock => 2,
            Self::LayerLock => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputChange {
    Wake,
    ShutDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct IdleState {
    elapsed_ms: u32,
    dimmed: bool,
    off: bool,
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
    layers: Layers<ROWS, COLUMNS>,
    overlays: Overlays,
    base_colors: KeyColorMap<ROWS, COLUMNS>,
    layer_colors: [Option<KeyColorMap<ROWS, COLUMNS>>; LAYER_COLOR_MAPS],
    shown_layer: u8,
    requested_layer: Option<u8>,
    brightness: u8,
    applied_brightness: u8,
    fade_level: u8,
    indicators: Indicators,
    indicator_keys: [Option<Key>; 4],
    locks: LockState,
    num_lock_inverted: bool,
    idle: Option<Idle>,
    idle_state: IdleState,
    host_control: bool,
    fast_scan: bool,
    output_enabled: bool,
    effect_stamp_ms: u32,
    frame_stamp_ms: u32,
    redraw_requested: bool,
    force_render: bool,
    save_due_ms: Option<u32>,
}

impl<H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>
where
    H: Lightbar + Signals,
{
    #[must_use]
    pub fn new(
        map: &'static LedMap<PAGES>,
        variant: &'static Variant<ROWS, COLUMNS>,
        geometry: Geometry,
        hooks: H,
        now_ms: u32,
    ) -> Self {
        let white = Color {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 255,
        };
        Self {
            map,
            variant,
            geometry,
            hooks,
            frame: Frame::new(),
            dirty: DirtyPages::new(),
            layers: Layers::new(),
            overlays: Overlays::new(),
            base_colors: [[BLACK; COLUMNS]; ROWS],
            layer_colors: [None; LAYER_COLOR_MAPS],
            shown_layer: 0,
            requested_layer: None,
            brightness: u8::MAX,
            applied_brightness: 0,
            fade_level: FADE_FULL,
            indicators: Indicators {
                caps_lock: white,
                scroll_lock: white,
                num_lock: white,
                layer_lock: white,
            },
            indicator_keys: [None; 4],
            locks: LockState::default(),
            num_lock_inverted: true,
            idle: None,
            idle_state: IdleState::default(),
            host_control: false,
            fast_scan: false,
            output_enabled: false,
            effect_stamp_ms: now_ms,
            frame_stamp_ms: now_ms,
            redraw_requested: false,
            force_render: false,
            save_due_ms: None,
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
        self.brightness
    }

    #[must_use]
    pub const fn fade_level(&self) -> u8 {
        self.fade_level
    }

    #[must_use]
    pub const fn is_idle_off(&self) -> bool {
        self.idle_state.off
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
    pub fn led_color(&self, led: Led) -> Option<Rgb> {
        self.map
            .position(led)
            .and_then(|position| self.frame.color(position))
    }

    #[must_use]
    pub fn key_color(&self, key: Key) -> Option<Rgb> {
        self.variant
            .led(key.row(), key.column())
            .and_then(|led| self.led_color(led))
    }

    pub fn set_variant(&mut self, variant: &'static Variant<ROWS, COLUMNS>) {
        self.variant = variant;
        self.paint_base_colors(true);
        self.refresh_lock_indicators(self.effect_stamp_ms);
    }

    pub fn apply_settings(&mut self, settings: &Settings, now_ms: u32) {
        self.brightness = settings.brightness;
        self.indicators = settings.indicators;
        self.layers.apply(&settings.effects);
        self.paint_base_colors(false);
        self.refresh_lock_indicators(now_ms);
    }

    pub fn apply_effects(&mut self, effects: &[Effect]) {
        self.layers.apply(effects);
        self.paint_base_colors(false);
    }

    pub fn set_base_colors(&mut self, colors: &KeyColors) {
        self.base_colors = color_map(colors);
        self.paint_base_colors(false);
    }

    pub fn set_layer_colors(&mut self, layer: u8, colors: Option<&KeyColors>) {
        let Some(slot) = usize::from(layer)
            .checked_sub(1)
            .and_then(|index| self.layer_colors.get_mut(index))
        else {
            return;
        };
        *slot = colors.map(color_map);
        if self.shown_layer == layer {
            self.requested_layer = Some(layer);
        }
    }

    pub const fn show_layer(&mut self, layer: u8) {
        self.requested_layer = Some(layer);
    }

    pub const fn set_idle(&mut self, idle: Option<Idle>) {
        self.idle = idle;
    }

    pub fn set_num_lock_inverted(&mut self, inverted: bool, now_ms: u32) {
        self.num_lock_inverted = inverted;
        self.refresh_lock_indicators(now_ms);
    }

    pub fn set_fast_scan(&mut self, fast_scan: bool) {
        if self.fast_scan != fast_scan {
            self.fast_scan = fast_scan;
            self.redraw_requested = true;
            self.force_render = true;
            self.paint_base_colors(false);
        }
    }

    pub fn set_indicator_key(&mut self, indicator: Indicator, key: Option<Key>, now_ms: u32) {
        if let Some(slot) = self.indicator_keys.get_mut(indicator.slot()) {
            *slot = key;
        }
        self.refresh_lock_indicators(now_ms);
    }

    pub fn update_lock_indicators(&mut self, locks: LockState, now_ms: u32) {
        self.locks = locks;
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
                fade(0, FADE_FULL, DRIVER_WAKE_FADE_MS),
                now_ms,
            );
        }
    }

    pub fn key_event(&mut self, key: Key, event: KeyEvent, now_ms: u32) {
        if !self.host_control {
            self.layers.event(key, event, now_ms, self.variant);
        }
    }

    pub fn release_all_keys(&mut self) {
        self.layers.release_all();
    }

    pub fn reset_idle_timer(&mut self, now_ms: u32) {
        if self.idle_state.dimmed || self.idle_state.off {
            let from = if self.idle_state.off { 0 } else { FADE_DIMMED };
            if !self.host_control {
                self.start_overlay(
                    OverlayId::IdleFade,
                    Span::Milliseconds(IDLE_WAKE_FADE_MS),
                    fade(from, FADE_FULL, IDLE_WAKE_FADE_MS),
                    now_ms,
                );
            }
        }
        self.idle_state = IdleState::default();
    }

    pub fn step_brightness(&mut self, increase: bool, now_ms: u32) -> u8 {
        self.brightness = if increase {
            let max = if self.fast_scan {
                BRIGHTNESS_MAX_FAST_SCAN
            } else {
                u8::MAX
            };
            self.brightness.saturating_add(BRIGHTNESS_STEP).min(max)
        } else {
            self.brightness.saturating_sub(BRIGHTNESS_STEP)
        };
        self.apply_brightness_change();
        self.hooks
            .brightness_changed(f32::from(self.brightness) * 100.0 / 255.0);
        self.save_due_ms = Some(now_ms.wrapping_add(BRIGHTNESS_SAVE_DELAY_MS));
        self.brightness
    }

    pub fn take_brightness_save(&mut self, now_ms: u32) -> Option<u8> {
        let due = self.save_due_ms?;
        if now_ms.wrapping_sub(due).cast_signed() < 0 {
            return None;
        }
        self.save_due_ms = None;
        Some(self.brightness)
    }

    pub fn enter_host_control(&mut self, now_ms: u32) {
        if self.host_control {
            return;
        }
        self.host_control = true;
        self.refresh_lock_indicators(now_ms);
        self.shown_layer = 0;
        self.requested_layer = None;
        self.paint_base_colors(false);
        self.force_render = true;
    }

    pub fn leave_host_control(&mut self, now_ms: u32) {
        if !self.host_control {
            return;
        }
        self.host_control = false;
        self.paint_base_colors(true);
        self.refresh_lock_indicators(now_ms);
        self.force_render = true;
    }

    pub fn set_host_key_color(&mut self, key: Key, color: Rgb, now_ms: u32) {
        self.enter_host_control(now_ms);
        self.main_canvas().set_key(key.row(), key.column(), color);
    }

    pub fn reset_host_key_color(&mut self, key: Key) {
        if let Some(color) = self
            .shown_colors()
            .get(key.row())
            .and_then(|row| row.get(key.column()))
        {
            let color = *color;
            self.main_canvas().set_key(key.row(), key.column(), color);
        }
    }

    pub fn set_host_colors(&mut self, colors: &KeyColors, now_ms: u32) {
        self.enter_host_control(now_ms);
        let colors = color_map::<ROWS, COLUMNS>(colors);
        let mut canvas = self.main_canvas();
        paint(&mut canvas, &colors);
    }

    pub fn update(&mut self, now_ms: u32, input: &impl KeyTravel) {
        if !self.host_control && self.layers.consumes_travel() {
            for row in 0..ROWS {
                for column in 0..COLUMNS {
                    let key = Key::new(index_u8(row), index_u8(column));
                    self.layers.feed(key, input.travel(key), input);
                }
            }
        }
        self.apply_layer_colors();
        if self.idle_update(now_ms) {
            return;
        }

        let force = self.force_render;
        let stamp_ms = if force {
            self.effect_stamp_ms = now_ms;
            now_ms
        } else {
            self.effect_stamp_ms
        };
        let since_ms = now_ms.wrapping_sub(stamp_ms);
        if !force && since_ms < FRAME_PERIOD_MS {
            return;
        }
        let since_ms = low_16(since_ms);
        let effects_enabled = self.effects_enabled();
        let effects_dirty =
            effects_enabled && (self.redraw_requested || self.layers.needs_render());
        if !self.overlays.needs_render(since_ms)
            && !effects_dirty
            && !self.hooks.needs_render(since_ms)
            && !force
        {
            return;
        }

        self.effect_stamp_ms = now_ms;
        let elapsed_ms = low_16(now_ms.wrapping_sub(stamp_ms));
        self.force_render = false;
        let fade_before = self.fade_level;
        if effects_enabled {
            self.redraw_requested = false;
            let mut scratch = Frame::<PAGES, LENGTH>::new();
            {
                let mut canvas = Painter::new(&mut scratch, self.map, self.variant, None);
                paint(&mut canvas, &self.base_colors);
                self.layers
                    .render(&mut canvas, self.geometry, u32::from(elapsed_ms));
                draw_overlays_and_lightbar(
                    &mut canvas,
                    &mut self.overlays,
                    &mut self.hooks,
                    &mut self.fade_level,
                    self.host_control,
                    &Pass::<ROWS, COLUMNS> {
                        restore: None,
                        classes: 0,
                        now_ms,
                        elapsed_ms,
                    },
                );
            }
            scratch.merge_into(&mut self.frame, &self.map.pages, &mut self.dirty);
        } else {
            let restore = *self.shown_colors();
            let mut canvas = Painter::new(
                &mut self.frame,
                self.map,
                self.variant,
                Some(&mut self.dirty),
            );
            draw_overlays_and_lightbar(
                &mut canvas,
                &mut self.overlays,
                &mut self.hooks,
                &mut self.fade_level,
                self.host_control,
                &Pass {
                    restore: Some(&restore),
                    classes: 0,
                    now_ms,
                    elapsed_ms,
                },
            );
        }
        if self.fade_level != fade_before
            && (self.hooks.brightness() | self.applied_brightness) != 0
        {
            self.dirty.push_all();
        }
    }

    pub fn next_write(&mut self) -> Option<PageWrite<LENGTH>> {
        loop {
            let page = self.dirty.pop()?;
            let (Some(layout), Some(linear)) = (self.map.page(page), self.frame.page(page)) else {
                continue;
            };
            let scale = output_scale(self.applied_brightness, self.fade_level);
            let mut write = PageWrite::new(page, layout, linear, scale);
            self.hooks
                .apply_page_brightness(page, linear, write.values_mut(), self.fade_level);
            return Some(write);
        }
    }

    pub fn invalidate(&mut self) {
        self.dirty.push_all();
    }

    #[must_use]
    pub fn has_pending_writes(&self) -> bool {
        !self.dirty.is_empty()
    }

    pub fn paint_base_colors(&mut self, force: bool) {
        self.redraw_requested = true;
        self.hooks.request_redraw();
        let covered = self.host_control || self.effects_shown() || self.shown_layer != 0;
        if force || !covered {
            let colors = self.base_colors;
            let mut canvas = self.main_canvas();
            paint(&mut canvas, &colors);
        }
        self.apply_brightness_change();
    }

    fn apply_brightness_change(&mut self) {
        if self.brightness != self.applied_brightness {
            self.applied_brightness = self.brightness;
            if self.fade_level != 0 {
                self.dirty.push_all();
            }
        }
        self.hooks.apply_brightness_change(self.brightness);
    }

    fn main_canvas(&mut self) -> Painter<'_, ROWS, COLUMNS, PAGES, LENGTH> {
        Painter::new(
            &mut self.frame,
            self.map,
            self.variant,
            Some(&mut self.dirty),
        )
    }

    fn shown_colors(&self) -> &KeyColorMap<ROWS, COLUMNS> {
        usize::from(self.shown_layer)
            .checked_sub(1)
            .and_then(|index| self.layer_colors.get(index))
            .and_then(Option::as_ref)
            .unwrap_or(&self.base_colors)
    }

    fn effects_shown(&self) -> bool {
        !self.fast_scan && !self.layers.is_empty()
    }

    fn effects_enabled(&self) -> bool {
        self.effects_shown() && self.shown_layer == 0 && !self.host_control
    }

    fn apply_layer_colors(&mut self) {
        let Some(layer) = self.requested_layer.take() else {
            return;
        };
        if let Some(colors) = usize::from(layer)
            .checked_sub(1)
            .and_then(|index| self.layer_colors.get(index))
            .copied()
            .flatten()
        {
            self.shown_layer = layer;
            let mut canvas = self.main_canvas();
            paint(&mut canvas, &colors);
            self.force_render = true;
            self.hooks.request_redraw();
            return;
        }
        if self.shown_layer != 0 {
            self.shown_layer = 0;
            self.paint_base_colors(false);
            self.force_render = true;
        }
    }

    fn idle_update(&mut self, now_ms: u32) -> bool {
        let previous_ms = self.frame_stamp_ms;
        self.frame_stamp_ms = now_ms;
        let Some(idle) = self.idle else {
            return false;
        };
        self.idle_state.elapsed_ms = self
            .idle_state
            .elapsed_ms
            .saturating_add(now_ms.wrapping_sub(previous_ms));
        let minutes = self.idle_state.elapsed_ms / MILLISECONDS_PER_MINUTE;
        let off_after = u32::from(idle.off_after_minutes);
        let dim_after = u32::from(idle.dim_after_minutes);
        if idle.off_enabled && minutes >= off_after {
            if !self.idle_state.off {
                let from = if dim_after == off_after {
                    FADE_FULL
                } else {
                    FADE_DIMMED
                };
                if !self.host_control {
                    self.start_overlay(
                        OverlayId::IdleFade,
                        Span::Forever,
                        fade(from, 0, IDLE_OFF_FADE_MS),
                        now_ms,
                    );
                }
                self.idle_state.off = true;
            }
            if self.overlays.idle_fade_done() {
                self.effect_stamp_ms = now_ms;
                return true;
            }
        } else if dim_after <= minutes && !self.idle_state.dimmed {
            if !self.host_control {
                self.start_overlay(
                    OverlayId::IdleFade,
                    Span::Forever,
                    fade(FADE_FULL, FADE_DIMMED, IDLE_DIM_FADE_MS),
                    now_ms,
                );
            }
            self.idle_state.dimmed = true;
        }
        false
    }

    fn refresh_lock_indicators(&mut self, now_ms: u32) {
        let locks = self.locks;
        for indicator in Indicator::ALL {
            let on = match indicator {
                Indicator::LayerLock => locks.layer_locked,
                Indicator::CapsLock => locks.caps_lock,
                Indicator::ScrollLock => locks.scroll_lock,
                Indicator::NumLock => locks.num_lock != self.num_lock_inverted,
            };
            self.set_lock_indicator(indicator, on, now_ms);
        }
    }

    fn set_lock_indicator(&mut self, indicator: Indicator, on: bool, now_ms: u32) {
        let id = indicator.overlay();
        let key = self.indicator_keys.get(indicator.slot()).copied().flatten();
        let hidden = indicator != Indicator::LayerLock && self.host_control;
        let Some(key) = key.filter(|_| on && !hidden) else {
            self.overlays.stop(id);
            return;
        };
        let color = opaque(match indicator {
            Indicator::CapsLock => self.indicators.caps_lock,
            Indicator::ScrollLock => self.indicators.scroll_lock,
            Indicator::NumLock => self.indicators.num_lock,
            Indicator::LayerLock => self.indicators.layer_lock,
        });
        if !self.host_control {
            self.start_overlay(id, Span::Forever, Overlay::Solid { key, color }, now_ms);
        }
    }
}

fn draw_overlays_and_lightbar<const ROWS: usize, const COLUMNS: usize>(
    canvas: &mut impl Canvas,
    overlays: &mut Overlays,
    hooks: &mut impl Lightbar,
    fade_level: &mut u8,
    host_control: bool,
    pass: &Pass<'_, ROWS, COLUMNS>,
) {
    let lightbar_classes = if hooks.owns_global_overlays() {
        CLASS_GLOBAL
    } else {
        0
    };
    if !host_control {
        let below = Pass {
            classes: CLASS_ALL & !lightbar_classes,
            ..*pass
        };
        overlays.render(canvas, hooks, &below, fade_level);
    }
    hooks.render(canvas, pass.elapsed_ms);
    if lightbar_classes != 0 && !host_control {
        let above = Pass {
            classes: lightbar_classes,
            ..*pass
        };
        overlays.render(canvas, hooks, &above, fade_level);
    }
}

fn paint<const ROWS: usize, const COLUMNS: usize>(
    canvas: &mut impl Canvas,
    colors: &KeyColorMap<ROWS, COLUMNS>,
) {
    for (row, colors) in colors.iter().enumerate() {
        for (column, &color) in colors.iter().enumerate() {
            canvas.set_key(row, column, color);
        }
    }
}

fn color_map<const ROWS: usize, const COLUMNS: usize>(
    colors: &KeyColors,
) -> KeyColorMap<ROWS, COLUMNS> {
    let mut map = [[BLACK; COLUMNS]; ROWS];
    for (index, slot) in map.iter_mut().flatten().enumerate() {
        *slot = colors.get(index).copied().unwrap_or(BLACK);
    }
    map
}

const fn fade(from: u8, to: u8, duration_ms: u16) -> Overlay {
    Overlay::Fade {
        from,
        to,
        duration_ms,
    }
}

fn index_u8(index: usize) -> u8 {
    u8::try_from(index).unwrap_or(u8::MAX)
}
