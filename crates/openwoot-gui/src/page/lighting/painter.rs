use gpui_kit::{
    Context, Div, IntoElement, Modifiers, ParentElement as _, Styled as _, Window,
    component::{
        Disableable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    div,
};
use openwoot_protocol::{keys::Layer, lighting::KeyColors};

use super::{BLACK, LightingPage, hsla_of, key_colors, palette::PRESETS, select};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    panel,
    store::Keyboard,
    usage,
};

impl LightingPage {
    pub(super) fn key_map(&self, window: &Window, cx: &Context<Self>) -> Div {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return div();
        };
        let (colors, inherited) = self.shown_colors();
        let page = cx.entity().downgrade();
        let view = KeyboardView::new("key-colors", keyboard, |index| KeyFace {
            legend: usage::legend(keyboard.legends.get(index).copied().unwrap_or_default()).into(),
            detail: None,
            fill: Some(hsla_of(
                colors
                    .and_then(|colors| colors.get(index))
                    .copied()
                    .unwrap_or(BLACK),
            )),
            travel: None,
            dimmed: inherited,
        })
        .selection(&self.selection)
        .fit_width(panel::content_width_beside_side(window))
        .on_key_click(move |key, event, window, cx| {
            let modifiers = event.modifiers();
            page.update(cx, |page, cx| page.click_key(key, modifiers, window, cx))
                .ok();
        });
        div().child(view)
    }

    pub(super) fn caption(&self, cx: &Context<Self>) -> Option<Div> {
        self.maps.as_ref()?;
        let text = if self.picking {
            "Click a key to pick its colour"
        } else if !self.selection.is_empty() {
            return None;
        } else if self.layer != Layer::Base && self.map(self.layer).is_none() {
            "Select keys to give this layer its own colours"
        } else {
            "Select keys to change their colour"
        };
        Some(
            h_flex()
                .w_full()
                .justify_center()
                .child(panel::label(text, cx)),
        )
    }

    pub(super) fn heading_actions(&self, cx: &Context<Self>) -> Div {
        h_flex()
            .gap_2()
            .child(
                Button::new("select-all-keys")
                    .label("Select all keys")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(keyboard) = this.store.read(cx).keyboard() {
                            let mut selection = this.selection.clone();
                            selection.select_all(keyboard);
                            this.selection = selection;
                            cx.notify();
                        }
                    })),
            )
            .child(
                Button::new("discard-selection")
                    .label("Discard selection")
                    .ghost()
                    .small()
                    .disabled(self.selection.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selection.clear();
                        cx.notify();
                    })),
            )
    }

    pub(super) fn preset_menu(cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        select("color-presets", "Pick a preset").dropdown_menu(move |menu, _, _| {
            PRESETS
                .iter()
                .enumerate()
                .fold(menu, |menu, (index, preset)| {
                    let page = page.clone();
                    menu.item(
                        PopupMenuItem::new(preset.name).on_click(move |_, window, cx| {
                            page.update(cx, |page, cx| page.apply_preset(index, window, cx))
                                .ok();
                        }),
                    )
                })
        })
    }

    pub(super) fn clear_layer(
        &mut self,
        layer: Layer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        if slot.take().is_some() {
            self.mark_layer(layer, window, cx);
        }
    }

    pub(super) fn paint_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() || self.maps.is_none() {
            return;
        }
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let reached = key_colors::reach(keyboard, self.selection.iter());
        let color = self.brush.rgb();
        self.edit_map(
            |colors| key_colors::paint(colors, &reached, color),
            window,
            cx,
        );
    }

    fn shown_colors(&self) -> (Option<&KeyColors>, bool) {
        match (self.map(self.layer), self.layer) {
            (Some(colors), _) => (Some(colors), false),
            (None, Layer::Base) => (None, false),
            (None, _) => (self.map(Layer::Base), true),
        }
    }

    fn click_key(
        &mut self,
        key: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.maps.is_none() {
            return;
        }
        if self.picking {
            let (colors, _) = self.shown_colors();
            let color = colors
                .and_then(|colors| colors.get(key))
                .copied()
                .unwrap_or(BLACK);
            self.picking = false;
            self.pick_brush(color, window, cx);
        } else {
            self.selection.click(key, modifiers);
        }
        cx.notify();
    }

    fn apply_preset(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
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
        .unwrap_or_else(|| key_colors::uniform(count, BLACK));
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        change(slot.get_or_insert(start));
        self.mark_layer(layer, window, cx);
    }
}
