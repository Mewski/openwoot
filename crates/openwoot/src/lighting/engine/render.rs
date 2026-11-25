use super::{FRAME_PERIOD_MS, Lighting};
use crate::{
    keys::Key,
    lighting::{
        canvas::{Canvas, Painter},
        frame::Frame,
        hooks::{Lightbar, Signals},
        input::KeyTravel,
        key_colors::{Shown, paint},
        output::{PageWrite, output_scale},
        overlay::{CLASS_ALL, CLASS_GLOBAL, OverlayId, Overlays, Pass},
        truncate::low_16,
    },
};

const EFFECT_ELAPSED_MAX_MS: u32 = 4 * FRAME_PERIOD_MS;

impl<H, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Lighting<H, ROWS, COLUMNS, PAGES, LENGTH>
where
    H: Lightbar + Signals,
{
    pub fn update(&mut self, now_ms: u32, input: &impl KeyTravel) {
        if self.hooks.take_brightness_change() && self.fade_level != 0 {
            self.dirty.push_all();
        }
        if !self.host_control && self.effects.consumes_travel() {
            for row in 0..ROWS {
                for column in 0..COLUMNS {
                    let key = Key::new(index_u8(row), index_u8(column));
                    self.effects.feed(key, input.travel(key), input);
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
            effects_enabled && (self.redraw_requested || self.effects.needs_render());
        let overlays_dirty = !self.host_control && self.overlays.needs_render(since_ms);
        if !overlays_dirty && !effects_dirty && !self.hooks.needs_render(since_ms) && !force {
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
                paint(&mut canvas, self.colors.base());
                self.effects.render(
                    &mut canvas,
                    self.geometry,
                    u32::from(elapsed_ms).min(EFFECT_ELAPSED_MAX_MS),
                );
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
            let restore = *self.colors.shown();
            let mut canvas = Painter::new(
                &mut self.frame,
                self.map,
                self.variant,
                Some(&mut self.dirty),
            );
            if !self.host_control && self.hooks.has_stale_keys() {
                paint(&mut canvas, &restore);
            }
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
            && (self.hooks.brightness() | self.brightness.applied()) != 0
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
            let scale = output_scale(self.brightness.applied(), self.fade_level);
            let mut write = PageWrite::new(page, layout, linear, scale);
            self.hooks
                .apply_page_brightness(page, linear, write.values_mut(), self.fade_level);
            if !self.sent.holds(&write) {
                return Some(write);
            }
        }
    }

    pub fn confirm_write(&mut self, write: &PageWrite<LENGTH>) {
        self.sent.record(write);
    }

    pub fn requeue_write(&mut self, write: &PageWrite<LENGTH>) {
        self.dirty.requeue(write.page);
    }

    pub fn invalidate(&mut self) {
        self.dirty.push_all();
        self.sent.forget();
    }

    fn apply_layer_colors(&mut self) {
        if self.host_control {
            return;
        }
        match self.colors.take_request() {
            Some(Shown::Layer(colors)) => {
                paint(&mut self.main_canvas(), &colors);
                self.force_render = true;
                self.hooks.request_redraw();
            }
            Some(Shown::Base) => {
                self.paint_base_colors(false);
                self.force_render = true;
            }
            None => {}
        }
    }

    fn idle_update(&mut self, now_ms: u32) -> bool {
        if self.host_control {
            return false;
        }
        let step = self.idle.advance(now_ms, self.fade_level);
        if let Some(fade) = step.fade {
            self.start_overlay(OverlayId::IdleFade, fade.span, fade.overlay, now_ms);
        }
        if step.off && self.fade_level == 0 && self.overlays.idle_fade_done() {
            self.effect_stamp_ms = now_ms;
            return true;
        }
        false
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
        overlays.render(canvas, &below, fade_level);
    }
    hooks.render(canvas, pass.elapsed_ms);
    if lightbar_classes != 0 && !host_control {
        let above = Pass {
            classes: lightbar_classes,
            ..*pass
        };
        overlays.render(canvas, &above, fade_level);
    }
}

fn index_u8(index: usize) -> u8 {
    u8::try_from(index).unwrap_or(u8::MAX)
}
