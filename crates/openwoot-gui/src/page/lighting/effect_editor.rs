use gpui_kit::{
    AppContext as _, Context, Div, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    keys::Layer,
    lighting::{Effect, MAX_EFFECTS},
};

use super::{
    LightingPage, add_button,
    effect::{self, BASE_KINDS, LAYER_KINDS, SCAN},
    palette::PRESETS,
    pill, remove_button, tile, tiles_box, wide_slider,
};
use crate::{
    dropdown::{Dropdown, Item},
    panel,
};

const BASE_COLUMN: f32 = 170.0;
const SELECTION_COLUMN: f32 = 300.0;
const BRIGHTNESS_WIDTH: f32 = 368.0;
const WIDE_CARD: f32 = 760.0;
const STATIC_DESCRIPTION: &str = "Keys show their own colours";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Focus {
    Base,
    Effect(usize),
}

#[derive(Debug, Clone)]
struct EffectDrag {
    index: usize,
    name: &'static str,
    icon: Catalog,
}

impl LightingPage {
    pub(super) fn effects_card(&self, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let brightness = self.brightness.read(cx).value().end();
        let main = self.layer == Layer::Base;
        let top = h_flex()
            .w_full()
            .gap_6()
            .items_start()
            .child(div().flex_1().min_w_0().child(panel::field(
                "Presets",
                Self::preset_menu(cx),
                cx,
            )))
            .child(
                div()
                    .flex_none()
                    .w(px(BRIGHTNESS_WIDTH))
                    .child(panel::field(
                        "Global Brightness",
                        wide_slider(&self.brightness, format!("{brightness:.0} %"), false, cx),
                        cx,
                    )),
            );
        let notice = if main {
            self.base()
                .map(|_| "The colour of the base effect overrides the colour of the keys.")
        } else {
            Some("RGB effects are only displayed and adjustable on the Main Layer.")
        };
        let header = v_flex()
            .w_full()
            .gap_4()
            .child(top)
            .children(notice.map(|notice| panel::warning(notice, cx)));
        let body = v_flex().w_full().gap_6().child(header);
        let card = panel::section("Effects", None, cx).p(px(super::CARD_PADDING));
        if !main {
            return Some(card.child(body));
        }
        let wide = f32::from(panel::content_width(window)) - super::COLOURS_WIDTH >= WIDE_CARD;
        let layers = v_flex()
            .flex_none()
            .w(px(BASE_COLUMN))
            .gap_8()
            .child(panel::field("Base Layer", self.base_pill(cx), cx))
            .child(panel::field(
                "Effect Layers",
                self.effect_layers(&settings.effects, cx),
                cx,
            ));
        let selection = panel::field("Effect Selection", self.effect_tiles(cx), cx);
        let parameters = panel::field("Effect Parameters", self.effect_parameters(cx), cx);
        let editor = if wide {
            h_flex()
                .w_full()
                .gap_4()
                .items_start()
                .child(layers)
                .child(div().flex_none().w(px(SELECTION_COLUMN)).child(selection))
                .child(div().flex_1().min_w_0().child(parameters))
        } else {
            h_flex().w_full().gap_4().items_start().child(layers).child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_6()
                    .child(selection)
                    .child(parameters),
            )
        };
        Some(card.child(body.child(editor)))
    }

    pub(super) fn focused_index(&self) -> Option<usize> {
        match self.focus {
            Focus::Base => self.base().map(|_| 0),
            Focus::Effect(index) => Some(index),
        }
    }

    pub(super) fn focused_effect(&self) -> Option<&Effect> {
        self.settings.as_ref()?.effects.get(self.focused_index()?)
    }

    pub(super) fn keep_focus(&mut self) {
        let count = self
            .settings
            .as_ref()
            .map_or(0, |settings| settings.effects.len());
        if let Focus::Effect(index) = self.focus
            && (index >= count || index < self.first_layer())
        {
            self.focus = Focus::Base;
        }
    }

    fn base(&self) -> Option<&Effect> {
        self.settings
            .as_ref()?
            .effects
            .first()
            .filter(|effect| effect::is_base(effect))
    }

    fn first_layer(&self) -> usize {
        usize::from(self.base().is_some())
    }

    fn preset_menu(cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        Dropdown::new("color-presets")
            .small()
            .placeholder("Pick a preset")
            .items(PRESETS.iter().map(|preset| Item::new(preset.name)))
            .on_select(move |index, window, cx| {
                page.update(cx, |page, cx| page.apply_preset(index, window, cx))
                    .ok();
            })
    }

    fn base_pill(&self, cx: &Context<Self>) -> Stateful<Div> {
        let (icon, name) = self.base().map_or((Catalog::Ban, "Static"), |effect| {
            (effect::icon(effect), effect::name(effect))
        });
        pill("base-effect", icon, name, self.focus == Focus::Base, cx)
            .on_click(cx.listener(|this, _, window, cx| this.focus_on(Focus::Base, window, cx)))
    }

    fn effect_layers(&self, effects: &[Effect], cx: &Context<Self>) -> Div {
        let first = self.first_layer();
        let primary = cx.theme().primary;
        v_flex()
            .w_full()
            .gap_2()
            .children(
                effects
                    .iter()
                    .enumerate()
                    .skip(first)
                    .map(|(index, effect)| {
                        let drag = EffectDrag {
                            index,
                            name: effect::name(effect),
                            icon: effect::icon(effect),
                        };
                        pill(
                            SharedString::from(format!("effect-layer-{index}")),
                            effect::icon(effect),
                            effect::name(effect),
                            self.focus == Focus::Effect(index),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.focus_on(Focus::Effect(index), window, cx);
                        }))
                        .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                        .drag_over::<EffectDrag>(move |style, _, _, _| style.border_color(primary))
                        .on_drop(cx.listener(move |this, drag: &EffectDrag, window, cx| {
                            this.move_effect(drag.index, index, window, cx);
                        }))
                        .child(
                            remove_button(format!("remove-effect-{index}")).on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.remove_effect(index, window, cx);
                                },
                            )),
                        )
                    }),
            )
            .when(effects.len() < MAX_EFFECTS, |this| {
                this.child(
                    add_button("add-effect-layer", "Add RGB Fx Layer", cx)
                        .on_click(cx.listener(|this, _, window, cx| this.add_layer(window, cx))),
                )
            })
    }

    fn effect_tiles(&self, cx: &Context<Self>) -> Div {
        let grid = div().grid().grid_cols(2).gap_2();
        let grid = match self.focus {
            Focus::Base => {
                let current = self.base().copied();
                let full = self
                    .settings
                    .as_ref()
                    .is_some_and(|settings| settings.effects.len() >= MAX_EFFECTS);
                let static_tile = tile(
                    "base-kind-static",
                    Catalog::Ban,
                    "Static",
                    STATIC_DESCRIPTION,
                    current.is_none(),
                    cx,
                )
                .on_click(cx.listener(|this, _, window, cx| this.choose_base(None, window, cx)));
                grid.child(static_tile)
                    .children(BASE_KINDS.iter().enumerate().map(|(index, kind)| {
                        let kind = *kind;
                        let chosen =
                            current.is_some_and(|effect| effect::same_kind(&effect, &kind));
                        tile(
                            SharedString::from(format!("base-kind-{index}")),
                            effect::icon(&kind),
                            effect::name(&kind),
                            effect::description(&kind),
                            chosen,
                            cx,
                        )
                        .when(full && current.is_none(), |this| this.opacity(0.5))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.choose_base(Some(kind), window, cx);
                            },
                        ))
                    }))
            }
            Focus::Effect(index) => {
                let current = self.focused_effect().copied();
                grid.children(LAYER_KINDS.iter().enumerate().map(|(position, kind)| {
                    let kind = *kind;
                    let chosen = current.is_some_and(|effect| effect::same_kind(&effect, &kind));
                    tile(
                        SharedString::from(format!("layer-kind-{position}")),
                        effect::icon(&kind),
                        effect::name(&kind),
                        effect::description(&kind),
                        chosen,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_kind(index, kind, window, cx);
                    }))
                }))
            }
        };
        tiles_box(cx).child(grid)
    }

    fn focus_on(&mut self, focus: Focus, window: &mut Window, cx: &mut Context<Self>) {
        self.focus = focus;
        self.sync_effect_controls(window, cx);
        cx.notify();
    }

    fn choose_base(&mut self, kind: Option<Effect>, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.base().copied();
        match (current, kind) {
            (None, None) => return,
            (Some(current), Some(kind)) if effect::same_kind(&current, &kind) => return,
            (Some(_), None) => self.edit_settings(
                |settings| {
                    settings.effects.remove(0);
                },
                window,
                cx,
            ),
            (Some(_), Some(kind)) => self.edit_effect(0, |effect| *effect = kind, window, cx),
            (None, Some(kind)) => self.edit_settings(
                |settings| {
                    settings.effects.insert(0, kind).ok();
                },
                window,
                cx,
            ),
        }
        self.focus = Focus::Base;
        self.sync_effect_controls(window, cx);
        cx.notify();
    }

    fn choose_kind(
        &mut self,
        index: usize,
        kind: Effect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self
            .settings
            .as_ref()
            .and_then(|settings| settings.effects.get(index))
            .is_some_and(|effect| !effect::same_kind(effect, &kind));
        if changed {
            self.edit_effect(index, |effect| *effect = kind, window, cx);
            self.sync_effect_controls(window, cx);
        }
    }

    fn add_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(count) = self
            .settings
            .as_ref()
            .map(|settings| settings.effects.len())
        else {
            return;
        };
        if count >= MAX_EFFECTS {
            return;
        }
        self.edit_settings(
            |settings| {
                settings.effects.push(SCAN).ok();
            },
            window,
            cx,
        );
        self.focus_on(Focus::Effect(count), window, cx);
    }

    fn remove_effect(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self
            .settings
            .as_ref()
            .map_or(0, |settings| settings.effects.len());
        if index >= count {
            return;
        }
        self.edit_settings(
            |settings| {
                settings.effects.remove(index);
            },
            window,
            cx,
        );
        self.focus = match self.focus {
            Focus::Effect(focused) if focused == index => {
                let first = self.first_layer();
                let remaining = count - 1;
                if index < remaining {
                    Focus::Effect(index)
                } else if index > first {
                    Focus::Effect(index - 1)
                } else {
                    Focus::Base
                }
            }
            Focus::Effect(focused) if focused > index => Focus::Effect(focused - 1),
            focus => focus,
        };
        self.keep_focus();
        self.sync_effect_controls(window, cx);
        cx.notify();
    }

    fn move_effect(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.first_layer();
        let count = self
            .settings
            .as_ref()
            .map_or(0, |settings| settings.effects.len());
        if from == to || from < first || to < first || from >= count || to >= count {
            return;
        }
        self.edit_settings(
            |settings| {
                let moved = settings.effects.remove(from);
                settings.effects.insert(to, moved).ok();
            },
            window,
            cx,
        );
        self.focus = Focus::Effect(to);
        self.sync_effect_controls(window, cx);
        cx.notify();
    }
}

impl Render for EffectDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(BASE_COLUMN)).shadow_lg().child(pill(
            "effect-drag",
            self.icon,
            self.name,
            true,
            cx,
        ))
    }
}
