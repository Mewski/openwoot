use heapless::Vec;
use openwoot_protocol::lightbar::{Entries, Settings, Signal};

use super::{
    paint::Strip,
    setup::{Pass, Setup},
};
use crate::lighting::{
    canvas::Canvas,
    config::Rgb,
    hooks::Lightbar,
    leds::{Led, LedMap, LedPosition},
    output::{output_scale, output_value},
    overlay::MAX_LIGHTBAR_LEDS,
    signals::{HeldKeys, KeyRate, Table},
};

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent states of the light bar, as in the reference firmware"
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
    brightness: u8,
    key_brightness: u8,
    brightness_changed: bool,
    redraw: bool,
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
            brightness: 0,
            key_brightness: u8::MAX,
            brightness_changed: false,
            redraw: false,
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
        self.active = false;
        if self.setup.profile.enabled {
            self.signals.clear();
            self.set_brightness(self.setup.profile.brightness.unwrap_or(self.key_brightness));
            self.active = self.setup.bind(&mut self.signals, self.fast_scan);
        } else {
            self.set_brightness(0);
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
        self.redraw || (self.active && self.setup.needs_render(since_render_ms, self.host_control))
    }

    fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u16) {
        self.redraw = false;
        if !self.setup.profile.enabled {
            return;
        }
        let strip = self.strip();
        strip.clear(canvas);
        let pass = Pass {
            strip,
            host_signals: self.signals.host_active(),
        };
        self.setup.render(canvas, &pass, elapsed_ms);
    }

    fn request_redraw(&mut self) {
        self.redraw = true;
    }

    fn brightness(&self) -> u8 {
        self.brightness
    }

    fn apply_brightness_change(&mut self, key_brightness: u8) {
        self.key_brightness = key_brightness;
        if self.setup.profile.enabled && self.setup.profile.brightness.is_none() {
            self.set_brightness(key_brightness);
        }
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

    fn draw_overlay(&mut self, canvas: &mut impl Canvas, colors: &[Rgb]) {
        for (led, color) in self.leds.iter().zip(colors) {
            canvas.set_led(*led, *color);
        }
    }

    fn clear_overlay(&mut self, canvas: &mut impl Canvas) {
        self.strip().clear(canvas);
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

    fn take_brightness_change(&mut self) -> bool {
        core::mem::take(&mut self.brightness_changed)
    }
}
