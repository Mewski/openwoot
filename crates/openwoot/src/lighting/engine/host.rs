use openwoot_protocol::{
    lightbar,
    lighting::{KeyColors, Rgb},
};

use super::Lighting;
use crate::{
    keys::Key,
    lighting::{
        canvas::{Canvas, Painter},
        color::BLACK,
        hooks::{Lightbar, Signals},
        key_colors::{color_map, paint},
        leds::Led,
        overlay::{FADE_FULL, OverlayId},
    },
};

impl<H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>
where
    H: Lightbar + Signals,
{
    pub fn enter_host_control(&mut self, now_ms: u32) {
        if self.host_control {
            return;
        }
        self.host_control = true;
        self.hooks.set_host_control(true);
        self.refresh_lock_indicators(now_ms);
        let restore = *self.colors.shown();
        let mut canvas = Painter::new(
            &mut self.frame,
            self.map,
            self.variant,
            Some(&mut self.dirty),
        );
        let mut finished_level = self.fade_level;
        self.overlays
            .finish_stopped(&mut canvas, &restore, &mut finished_level);
        self.overlays.remove(OverlayId::IdleFade);
        self.overlays.remove(OverlayId::WakeFade);
        self.idle.restart(now_ms);
        self.set_fade_level(FADE_FULL);
        let layer = self.colors.shown_layer();
        self.colors.hide_layers();
        if layer != 0 {
            self.colors.request(layer);
        }
        self.paint_base_colors(false);
        self.force_render = true;
    }

    pub fn leave_host_control(&mut self, now_ms: u32) {
        self.reset_host_lightbar();
        if !self.host_control {
            return;
        }
        self.host_control = false;
        self.hooks.set_host_control(false);
        self.idle.restart(now_ms);
        self.paint_base_colors(true);
        self.refresh_lock_indicators(now_ms);
        self.force_render = true;
    }

    pub fn set_host_key_color(&mut self, key: Key, color: Rgb, now_ms: u32) {
        self.enter_host_control(now_ms);
        self.main_canvas().set_key(key.row(), key.column(), color);
    }

    pub fn set_host_colors(&mut self, colors: &KeyColors, now_ms: u32) {
        self.enter_host_control(now_ms);
        let colors = color_map::<ROWS, COLUMNS>(colors);
        paint(&mut self.main_canvas(), &colors);
    }

    pub fn reset_host_key_color(&mut self, key: Key) {
        if !self.host_control {
            return;
        }
        if let Some(&color) = self
            .colors
            .shown()
            .get(key.row())
            .and_then(|row| row.get(key.column()))
        {
            self.main_canvas().set_key(key.row(), key.column(), color);
        }
    }

    pub fn reset_host_colors(&mut self) {
        if !self.host_control {
            return;
        }
        let colors = *self.colors.shown();
        paint(&mut self.main_canvas(), &colors);
    }

    pub fn set_host_lightbar(&mut self, colors: &[Rgb]) {
        self.hooks.set_host_colors(colors);
    }

    pub fn reset_host_lightbar(&mut self) {
        self.hooks.reset_host_colors();
    }

    #[must_use]
    pub fn host_lightbar(&self) -> bool {
        self.hooks.host_driven()
    }

    pub fn shown_keys(&self, colors: &mut KeyColors) {
        colors.clear();
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let color = self
                    .variant
                    .led(row, column)
                    .map_or(BLACK, |led| self.shown_led(led));
                if colors.push(color).is_err() {
                    return;
                }
            }
        }
    }

    pub fn shown_lightbar(&self, colors: &mut lightbar::Colors) {
        colors.clear();
        for led in self.hooks.leds() {
            if colors.push(self.shown_led(*led)).is_err() {
                return;
            }
        }
    }

    fn shown_led(&self, led: Led) -> Rgb {
        if self.idle.is_off() {
            return BLACK;
        }
        self.map
            .position(led)
            .and_then(|position| self.frame.color(position))
            .unwrap_or(BLACK)
    }
}
