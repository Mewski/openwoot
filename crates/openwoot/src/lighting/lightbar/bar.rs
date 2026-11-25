use heapless::Vec;
use openwoot_protocol::{
    lightbar::{Builtin, Entries, Settings, Signal},
    lighting::Rgb,
};

use super::{
    paint::Strip,
    setup::{Pass, Setup},
};
use crate::lighting::{
    canvas::Canvas,
    hooks::Lightbar,
    leds::{Led, LedMap, LedPosition},
    output::{output_scale, output_value},
    overlay::MAX_LIGHTBAR_LEDS,
    signals::{HeldKeys, KeyRate, Table},
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent states of the light bar"
)]
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    leds: &'static [Led],
    positions: Vec<Option<LedPosition>, MAX_LIGHTBAR_LEDS>,
    columns: u8,
    pub(super) setup: Setup,
    pub(super) signals: Table,
    pub(super) key_rate: KeyRate,
    pub(super) held: HeldKeys,
    host_colors: Option<Vec<Rgb, MAX_LIGHTBAR_LEDS>>,
    brightness: u8,
    key_brightness: u8,
    brightness_changed: bool,
    redraw: bool,
    stale_keys: bool,
    active: bool,
    fast_scan: bool,
    host_control: bool,
}

impl Bar {
    #[must_use]
    pub fn new<const PAGES: usize>(
        leds: &'static [Led],
        map: &LedMap<PAGES>,
        columns: usize,
    ) -> Self {
        let leds = leds.get(..MAX_LIGHTBAR_LEDS).unwrap_or(leds);
        Self {
            leds,
            positions: leds.iter().map(|led| map.position(*led)).collect(),
            columns: u8::try_from(columns).unwrap_or(u8::MAX),
            setup: Setup::new(),
            signals: Table::new(),
            key_rate: KeyRate::new(0),
            held: HeldKeys::new(),
            host_colors: None,
            brightness: 0,
            key_brightness: u8::MAX,
            brightness_changed: false,
            redraw: false,
            stale_keys: false,
            active: false,
            fast_scan: false,
            host_control: false,
        }
    }

    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.setup.profile
    }

    #[must_use]
    pub const fn global(&self) -> &Entries {
        &self.setup.global
    }

    pub fn publish(&mut self, signal: Signal, value: f32) {
        let subscribers = match signal {
            Signal::Builtin(builtin) => self.signals.publish_if_changed(builtin, value),
            Signal::Host(id) => self.signals.set_host(id, value),
        };
        for subscriber in subscribers {
            self.setup.notify(*subscriber, signal, value);
        }
    }

    const fn strip(&self) -> Strip<'static> {
        Strip {
            leds: self.leds,
            columns: self.columns,
        }
    }

    fn config_changed(&mut self) {
        self.redraw = true;
        self.stale_keys |= self.setup.forget_painted_keys();
        self.active = false;
        if self.setup.profile.enabled {
            self.signals.clear();
            self.active = self.setup.bind(&mut self.signals, self.fast_scan);
        }
        self.set_brightness(self.shown_brightness());
    }

    fn shown_brightness(&self) -> u8 {
        if self.setup.profile.enabled || self.host_colors.is_some() {
            self.setup.profile.brightness.unwrap_or(self.key_brightness)
        } else {
            0
        }
    }

    const fn set_brightness(&mut self, brightness: u8) {
        if self.brightness != brightness {
            self.brightness = brightness;
            self.brightness_changed = true;
        }
    }
}

impl Lightbar for Bar {
    fn needs_render(&self, since_render_ms: u16) -> bool {
        self.redraw
            || (self.active
                && self.host_colors.is_none()
                && self.setup.needs_render(since_render_ms, self.host_control))
    }

    fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u16) {
        self.redraw = false;
        self.stale_keys = false;
        let strip = self.strip();
        if let Some(colors) = &self.host_colors {
            strip.show(canvas, colors);
            return;
        }
        strip.clear(canvas);
        if !self.setup.profile.enabled {
            return;
        }
        let pass = Pass {
            strip,
            host_signals: self.signals.host_active(),
        };
        if self.setup.render(canvas, &pass, elapsed_ms) {
            self.stale_keys = true;
            self.redraw = true;
        }
    }

    fn request_redraw(&mut self) {
        self.redraw = true;
    }

    fn brightness(&self) -> u8 {
        self.brightness
    }

    fn apply_brightness_change(&mut self, key_brightness: u8) {
        self.key_brightness = key_brightness;
        self.set_brightness(self.shown_brightness());
        self.publish(
            Signal::Builtin(Builtin::Brightness),
            f32::from(key_brightness) * 100.0 / 255.0,
        );
    }

    fn apply_page_brightness(&self, page: usize, linear: &[u8], output: &mut [u8], fade_level: u8) {
        let scale = output_scale(self.brightness, fade_level);
        for position in self.positions.iter().flatten() {
            if usize::from(position.page) != page {
                continue;
            }
            let offset = usize::from(position.offset);
            for channel in offset..offset + 3 {
                if let (Some(linear), Some(output)) = (linear.get(channel), output.get_mut(channel))
                {
                    *output = output_value(*linear, scale);
                }
            }
        }
    }

    fn owns_global_overlays(&self) -> bool {
        true
    }

    fn has_stale_keys(&self) -> bool {
        self.stale_keys
    }

    fn configure(&mut self, profile: &Settings, global: &Entries, key_brightness: u8) {
        self.setup.profile.clone_from(profile);
        self.setup.global.clone_from(global);
        self.key_brightness = key_brightness;
        self.config_changed();
    }

    fn set_fast_scan(&mut self, fast_scan: bool) {
        if self.fast_scan != fast_scan {
            self.fast_scan = fast_scan;
            self.config_changed();
        }
    }

    fn set_host_control(&mut self, host_control: bool) {
        self.host_control = host_control;
    }

    fn set_host_colors(&mut self, colors: &[Rgb]) {
        if self.host_colors.is_none() {
            self.stale_keys |= self.setup.forget_painted_keys();
        }
        let mut shown = Vec::<Rgb, MAX_LIGHTBAR_LEDS>::new();
        let _ = shown.extend_from_slice(colors.get(..self.leds.len()).unwrap_or(colors));
        self.host_colors = Some(shown);
        self.redraw = true;
        self.set_brightness(self.shown_brightness());
    }

    fn reset_host_colors(&mut self) {
        if self.host_colors.take().is_some() {
            self.redraw = true;
            self.set_brightness(self.shown_brightness());
        }
    }

    fn host_driven(&self) -> bool {
        self.host_colors.is_some()
    }

    fn leds(&self) -> &[Led] {
        self.leds
    }

    fn take_brightness_change(&mut self) -> bool {
        core::mem::take(&mut self.brightness_changed)
    }
}
