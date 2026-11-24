use gpui_kit::{
    App, Context, Div, Modifiers, ParentElement as _, SharedString, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        Disableable as _, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        color_picker::ColorPicker,
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        tab::{Tab, TabBar},
    },
    div, px,
};
use openwoot_protocol::{
    keys::Layer,
    lighting::{KeyColors, Rgb},
};

use super::{BLACK, LightingPage, WHITE, hex, hsla_of, key_colors, palette::PRESETS, rgb_of};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    panel,
    store::Keyboard,
    usage,
};

const KEYBOARD_MARGIN: f32 = 360.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Select,
    Paint,
    Pick,
}

impl Tool {
    const ALL: [Self; 3] = [Self::Select, Self::Paint, Self::Pick];

    const fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Paint => "Paint",
            Self::Pick => "Eye dropper",
        }
    }

    const fn icon(self) -> Catalog {
        match self {
            Self::Select => Catalog::MousePointer,
            Self::Paint => Catalog::Paintbrush,
            Self::Pick => Catalog::Pipette,
        }
    }

    const fn id(self) -> &'static str {
        match self {
            Self::Select => "tool-select",
            Self::Paint => "tool-paint",
            Self::Pick => "tool-pick",
        }
    }

    const fn hint(self) -> &'static str {
        match self {
            Self::Select => {
                "Click keys to select them, with Shift or Cmd to add keys; a colour chosen in \
                 the picker paints the selection."
            }
            Self::Paint => "Click a key to paint it in the colour.",
            Self::Pick => "Click a key to take its colour.",
        }
    }
}

impl LightingPage {
    pub(super) fn colors_section(&self, window: &Window, cx: &Context<Self>) -> Div {
        panel::section(
            "Key colours",
            Some(
                "The colour of every key, per layer. A Fn layer with its own colours shows them \
                 while the layer is held; one without shows the base layer's colours. LEDs that \
                 have no key of their own, such as the space bar's extra LEDs, take the colour \
                 of the key whose width covers them."
                    .into(),
            ),
            cx,
        )
        .child(self.layer_tabs(cx))
        .child(self.layer_note(cx))
        .child(self.tool_bar(cx))
        .child(self.action_bar(cx))
        .child(self.key_map(window, cx))
    }

    fn layer_tabs(&self, cx: &Context<Self>) -> TabBar {
        let selected = Layer::ALL
            .iter()
            .position(|layer| *layer == self.layer)
            .unwrap_or(0);
        TabBar::new("color-layers")
            .segmented()
            .selected_index(selected)
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                if let Some(layer) = Layer::ALL.get(*index) {
                    this.layer = *layer;
                    cx.notify();
                }
            }))
            .children(Layer::ALL.map(|layer| {
                let own = self.map(layer).is_some();
                let label = if own || layer == Layer::Base {
                    usage::layer_name(layer).to_owned()
                } else {
                    format!("{} (base colours)", usage::layer_name(layer))
                };
                Tab::new().label(label)
            }))
    }

    fn layer_note(&self, cx: &Context<Self>) -> Div {
        let text = match (self.layer, self.map(self.layer).is_some()) {
            (Layer::Base, false) => {
                "The keys of this profile are unlit. Painting a key gives the base layer colours."
            }
            (Layer::Base, true) => {
                "Turn the colours off to leave every key unlit while no effect paints it."
            }
            (_, false) => {
                "This layer shows the base layer's colours while it is held. Painting a key \
                 gives the layer its own colours, starting from the base layer's."
            }
            (_, true) => {
                "This layer shows its own colours while it is held; effects pause meanwhile."
            }
        };
        let hint = self.tool.hint();
        panel::note(
            if self.maps.is_some() {
                format!("{text} {hint}")
            } else {
                hint.to_owned()
            },
            cx,
        )
    }

    fn tool_bar(&self, cx: &Context<Self>) -> Div {
        let ready = self.maps.is_some();
        h_flex()
            .gap_2()
            .items_center()
            .flex_wrap()
            .children(Tool::ALL.map(|tool| {
                Button::new(tool.id())
                    .icon(tool.icon())
                    .label(tool.label())
                    .small()
                    .outline()
                    .selected(self.tool == tool)
                    .disabled(!ready)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.tool = tool;
                        cx.notify();
                    }))
            }))
            .child(div().w(px(12.0)))
            .child(ColorPicker::new(&self.paint).small())
            .child(
                div()
                    .text_sm()
                    .font_family("monospace")
                    .child(hex(self.paint_color(cx))),
            )
    }

    fn action_bar(&self, cx: &Context<Self>) -> Div {
        let ready = self.maps.is_some();
        let selected = self.selection.len();
        let own = self.map(self.layer).is_some();
        let page = cx.entity().downgrade();
        h_flex()
            .gap_2()
            .items_center()
            .flex_wrap()
            .child(
                Button::new("paint-selection")
                    .icon(Catalog::PaintBucket)
                    .label(SharedString::from(match selected {
                        0 => "Paint selection".to_owned(),
                        count => format!("Paint {count} selected"),
                    }))
                    .small()
                    .primary()
                    .disabled(!ready || selected == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.paint_selection(window, cx);
                    })),
            )
            .child(
                Button::new("select-all")
                    .label("Select all")
                    .small()
                    .ghost()
                    .disabled(!ready)
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
                Button::new("clear-selection")
                    .label("Clear selection")
                    .small()
                    .ghost()
                    .disabled(selected == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selection.clear();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("fill-all")
                    .icon(Catalog::PaintBucket)
                    .label("Fill all keys")
                    .small()
                    .outline()
                    .disabled(!ready)
                    .on_click(cx.listener(|this, _, window, cx| this.fill(window, cx))),
            )
            .child(
                Button::new("color-presets")
                    .icon(Catalog::Sparkles)
                    .label("Presets")
                    .small()
                    .outline()
                    .dropdown_caret(true)
                    .disabled(!ready)
                    .dropdown_menu(move |menu, _, _| {
                        PRESETS
                            .iter()
                            .enumerate()
                            .fold(menu, |menu, (index, preset)| {
                                let page = page.clone();
                                menu.item(PopupMenuItem::new(preset.name).on_click(
                                    move |_, window, cx| {
                                        page.update(cx, |page, cx| {
                                            page.apply_preset(index, window, cx);
                                        })
                                        .ok();
                                    },
                                ))
                            })
                    }),
            )
            .child(
                Button::new("clear-layer")
                    .icon(Catalog::Eraser)
                    .label(if self.layer == Layer::Base {
                        "Turn colours off"
                    } else {
                        "Use base colours"
                    })
                    .small()
                    .ghost()
                    .disabled(!ready || !own)
                    .on_click(cx.listener(|this, _, window, cx| this.clear_layer(window, cx))),
            )
    }

    fn key_map(&self, window: &Window, cx: &Context<Self>) -> Div {
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
        .fit_width(window.viewport_size().width - px(KEYBOARD_MARGIN))
        .on_key_click(move |key, event, window, cx| {
            let modifiers = event.modifiers();
            page.update(cx, |page, cx| page.click_key(key, modifiers, window, cx))
                .ok();
        });
        div().pt_2().child(view)
    }

    fn shown_colors(&self) -> (Option<&KeyColors>, bool) {
        match (self.map(self.layer), self.layer) {
            (Some(colors), _) => (Some(colors), false),
            (None, Layer::Base) => (None, false),
            (None, _) => (self.map(Layer::Base), true),
        }
    }

    fn paint_color(&self, cx: &App) -> Rgb {
        self.paint.read(cx).value().map_or(WHITE, rgb_of)
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
        match self.tool {
            Tool::Select => {
                self.selection.click(key, modifiers);
                cx.notify();
            }
            Tool::Paint => self.paint_keys([key], window, cx),
            Tool::Pick => {
                let (colors, _) = self.shown_colors();
                let color = colors
                    .and_then(|colors| colors.get(key))
                    .copied()
                    .unwrap_or(BLACK);
                self.paint.update(cx, |picker, cx| {
                    picker.set_value(hsla_of(color), window, cx);
                });
                self.tool = Tool::Paint;
                cx.notify();
            }
        }
    }

    pub(super) fn paint_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() || self.maps.is_none() {
            return;
        }
        let keys: Vec<usize> = self.selection.iter().collect();
        self.paint_keys(keys, window, cx);
    }

    fn paint_keys(
        &mut self,
        keys: impl IntoIterator<Item = usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let reached = key_colors::reach(keyboard, keys);
        let color = self.paint_color(cx);
        self.edit_map(
            |colors| key_colors::paint(colors, &reached, color),
            window,
            cx,
        );
    }

    fn fill(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let color = self.paint_color(cx);
        self.edit_map(
            |colors| {
                for slot in colors.iter_mut() {
                    *slot = color;
                }
            },
            window,
            cx,
        );
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

    fn clear_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        if let Some(slot) = self.map_slot(layer) {
            *slot = None;
            self.mark_layer(layer, window, cx);
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
        .unwrap_or_else(|| key_colors::uniform(count, BLACK));
        let Some(slot) = self.map_slot(layer) else {
            return;
        };
        change(slot.get_or_insert(start));
        self.mark_layer(layer, window, cx);
    }
}
