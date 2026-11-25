use gpui_kit::{Context, Hsla, MouseButton, Window, hsla};
use openwoot_protocol::{
    keys::Layer,
    lighting::{Color, KeyColors, Rgb},
};

use super::{LightingPage, key_colors, palette::PRESETS};
use crate::{
    color_picker::{ColorPicker, hsla_of},
    keyboard::{KeyFace, KeyboardView},
    store::Keyboard,
    usage,
};

const INHERITED_OPACITY: f32 = 0.45;

impl LightingPage {
    pub(super) fn key_map(&self, cx: &Context<Self>) -> Option<KeyboardView> {
        let keyboard = self.store.read(cx).keyboard()?;
        let (colors, inherited) = self.shown_colors();
        let indicators = self.settings.as_ref().map(|settings| settings.indicators);
        let locks = self.locks;
        let clicked = cx.entity().downgrade();
        let swept = clicked.clone();
        let picked = clicked.clone();
        let pressed = clicked.clone();
        let view = KeyboardView::new("key-colors", keyboard, |index| {
            let color = colors
                .and_then(|colors| colors.get(index))
                .copied()
                .unwrap_or(key_colors::BLACK);
            let fill = hsla_of(color);
            KeyFace {
                legend: usage::legend(keyboard.legends.get(index).copied().unwrap_or_default())
                    .into(),
                fill: Some(if inherited {
                    fill.opacity(INHERITED_OPACITY)
                } else {
                    fill
                }),
                dimmed: inherited,
                marker: locks
                    .lock_on(index)
                    .zip(indicators)
                    .map(|(lock, indicators)| marker_of(lock.color(&indicators))),
                tooltip: locks.lock_on(index).map(|lock| lock.name().into()),
                ..KeyFace::default()
            }
        })
        .on_key_click(move |key, _, window, cx| {
            clicked
                .update(cx, |page, cx| page.paint_key(key, window, cx))
                .ok();
        })
        .on_key_sweep(move |key, window, cx| {
            swept
                .update(cx, |page, cx| page.paint_key(key, window, cx))
                .ok();
        })
        .on_key_secondary_click(move |key, window, cx| {
            picked
                .update(cx, |page, cx| page.pick_key(key, window, cx))
                .ok();
        })
        .on_marker_press(move |key, button, window, cx| {
            pressed
                .update(cx, |page, cx| page.press_marker(key, button, window, cx))
                .ok();
        });
        Some(view)
    }

    pub(super) fn caption(&self) -> Option<&'static str> {
        self.maps.as_ref()?;
        (self.layer != Layer::Base && self.map(self.layer).is_none())
            .then_some("Paint keys to give this layer its own colours")
    }

    pub(super) fn apply_preset(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let rows = usize::from(keyboard.capabilities.key_rows);
        let columns = keyboard.columns();
        let Some(preset) = PRESETS.get(index) else {
            return;
        };
        let painted = preset.key_colors(rows, columns);
        self.edit_map(|colors| *colors = painted, window, cx);
    }

    pub(super) fn copy_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        self.copied = self.colors_of(layer).cloned();
        cx.notify();
    }

    pub(super) fn paste_layer(
        &mut self,
        layer: Layer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(copied) = self.copied.clone() else {
            return;
        };
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        if slot.as_ref() == Some(&copied) {
            return;
        }
        *slot = Some(copied);
        self.mark_layer(layer, window, cx);
    }

    pub(super) fn clear_layer(
        &mut self,
        layer: Layer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if layer == Layer::Base {
            return;
        }
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        if slot.take().is_some() {
            self.mark_layer(layer, window, cx);
        }
    }

    pub(super) fn shown_colors(&self) -> (Option<&KeyColors>, bool) {
        match (self.map(self.layer), self.layer) {
            (Some(colors), _) => (Some(colors), false),
            (None, Layer::Base) => (None, false),
            (None, _) => (self.map(Layer::Base), true),
        }
    }

    fn colors_of(&self, layer: Layer) -> Option<&KeyColors> {
        self.map(layer).or_else(|| self.map(Layer::Base))
    }

    fn paint_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.maps.is_none() {
            return;
        }
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let reached = key_colors::reach(keyboard, [key]);
        let brush = self.brush.read(cx).value();
        let color = brush.unwrap_or(key_colors::BLACK);
        self.edit_map(
            |colors| key_colors::paint(colors, &reached, color),
            window,
            cx,
        );
        if let Some(color) = brush {
            ColorPicker::remember(color, cx);
        }
    }

    fn pick_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        let (colors, _) = self.shown_colors();
        let color = colors
            .and_then(|colors| colors.get(key))
            .copied()
            .unwrap_or(key_colors::BLACK);
        self.brush
            .update(cx, |brush, cx| brush.set_value(color, window, cx));
        cx.notify();
    }

    fn press_marker(
        &mut self,
        key: usize,
        button: MouseButton,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(lock) = self.locks.lock_on(key) else {
            return;
        };
        match button {
            MouseButton::Right => {
                let Some(color) = self
                    .settings
                    .as_ref()
                    .map(|settings| lock.color(&settings.indicators))
                else {
                    return;
                };
                let picked = (color.alpha != 0).then_some(color.rgb());
                self.brush
                    .update(cx, |brush, cx| brush.set_value(picked, window, cx));
                cx.notify();
            }
            MouseButton::Left => {
                let brush = self.brush.read(cx).value();
                let color = brush.map_or(Color::TRANSPARENT, Rgb::opaque);
                self.edit_settings(
                    |settings| lock.set(&mut settings.indicators, color),
                    window,
                    cx,
                );
                if let Some(color) = brush {
                    ColorPicker::remember(color, cx);
                }
            }
            MouseButton::Middle | MouseButton::Navigate(_) => {}
        }
    }

    fn edit_map(
        &mut self,
        change: impl FnOnce(&mut KeyColors),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(count) = self.store.read(cx).keyboard().map(Keyboard::key_count) else {
            return;
        };
        let layer = self.layer;
        let start = match layer {
            Layer::Base => None,
            Layer::One | Layer::Two | Layer::Three => self.map(Layer::Base).cloned(),
        }
        .unwrap_or_else(|| key_colors::uniform(count, key_colors::BLACK));
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        let before = slot.clone();
        change(slot.get_or_insert(start));
        if *slot != before {
            self.mark_layer(layer, window, cx);
        }
    }
}

fn marker_of(color: Color) -> Hsla {
    if color.alpha == 0 {
        hsla(0.0, 0.0, 0.0, 0.0)
    } else {
        hsla_of(color.rgb())
    }
}
