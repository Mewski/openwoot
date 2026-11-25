use gpui_kit::{
    AnyElement, App, Context, Div, ElementId, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        slider::Slider,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, Interrupt, ModTap, Resolution, Socd},
    keys::{Action, TapDuration},
};

use super::{
    ActionSlot, AdvancedPage, Kind, Tab,
    creation::{help, warning},
    kind::{self, Choice},
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::Dragged,
    page::remap::keycap_face,
    panel,
    store::Keyboard,
    switch::Switch,
    theme,
};

const VALUE_WIDTH: f32 = 72.0;
const RADIO_SIZE: f32 = 18.0;
const SLOT_SIZE: f32 = 60.0;
const CALLOUT_OFFSET: f32 = 34.0;
const BIND_PROMPT: &str = "Press any key to bind";
const SOCD_WARNING: &str = "Snappy Tappy (SOCD) is prohibited in some games, such as \
                            Counter-Strike 2; using it in competitive play may lead to penalties";
const PROFILE_WIDE: &str = "Changes here apply to every key of this profile";
const TICK_RATE_HELP: &str = "Some games read keys once per frame and miss very short taps; a \
                              lower tick rate holds taps longer";

impl AdvancedPage {
    pub(super) fn editor_card(
        &self,
        entry: &advanced::Key,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let (border, muted, radius, surface) = {
            let theme = cx.theme();
            (
                theme.border,
                theme.muted_foreground,
                theme.radius_lg,
                theme.group_box,
            )
        };
        let kind = Kind::of(&entry.behaviour);
        let entry = *entry;
        let header = h_flex()
            .w_full()
            .px_6()
            .py_4()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(kind.title()),
            )
            .child(help(
                SharedString::from(format!("editor-help-{kind:?}")),
                kind.help(),
                cx,
            ))
            .child(div().flex_1())
            .child(
                Button::new("advanced-delete")
                    .icon(Catalog::Trash)
                    .label("Delete")
                    .on_click(cx.listener(move |_, _, window, cx| {
                        Self::confirm_delete(entry, window, cx);
                    })),
            )
            .child(
                Button::new("advanced-done")
                    .primary()
                    .label("Done")
                    .on_click(cx.listener(|this, _, _, cx| this.overview(cx))),
            );
        let left = v_flex()
            .flex_1()
            .min_w_0()
            .p_6()
            .gap_4()
            .child(div().text_sm().text_color(muted).child(kind.help()))
            .child(self.settings_of(&entry, keyboard, cx));
        let right = v_flex()
            .flex_1()
            .min_w_0()
            .p_6()
            .gap_4()
            .border_l_1()
            .border_color(border)
            .child(self.tabs(kind, cx))
            .child(self.tab_content(&entry, kind, cx));
        v_flex()
            .w_full()
            .rounded(radius)
            .bg(surface)
            .child(header)
            .child(h_flex().w_full().items_stretch().child(left).child(right))
    }

    fn settings_of(
        &self,
        entry: &advanced::Key,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => self
                .timeline(entry, keystroke, keyboard, cx)
                .into_any_element(),
            Behaviour::ModTap(mod_tap) => h_flex()
                .w_full()
                .pt_4()
                .gap_4()
                .justify_center()
                .child(self.labelled_slot("Hold", ActionSlot::Hold, mod_tap.hold, cx))
                .child(self.labelled_slot("Tap", ActionSlot::Tap, mod_tap.tap, cx))
                .into_any_element(),
            Behaviour::Toggle(action) => h_flex()
                .w_full()
                .pt_4()
                .justify_center()
                .child(self.labelled_slot("Action", ActionSlot::Toggle, action, cx))
                .into_any_element(),
            Behaviour::DepthPriority(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                let choices = kind::bottomed_out_resolutions(&first, &second);
                panel::field(
                    "When both keys are fully pressed",
                    resolution_dropdown("depth-bottomed-out", choices, pair.both_bottomed_out, cx),
                    cx,
                )
                .into_any_element()
            }
            Behaviour::Socd(pair) => self.socd_settings(entry, pair, cx).into_any_element(),
        }
    }

    fn socd_settings(&self, entry: &advanced::Key, pair: Socd, cx: &mut Context<Self>) -> Div {
        let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
        let choices = kind::socd_resolutions(&first, &second);
        let cards: Vec<Stateful<Div>> = choices
            .into_iter()
            .map(|choice| {
                let value = choice.value;
                radio_card(
                    SharedString::from(format!("socd-{value:?}")),
                    &choice,
                    value == pair.resolution,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_resolution(value, window, cx);
                }))
            })
            .collect();
        v_flex()
            .gap_4()
            .child(section_title(
                "SOCD cleaning behaviour",
                "What happens when both keys are pressed",
                cx,
            ))
            .child(v_flex().gap_2().children(cards))
            .child(panel::toggle(
                "Alternative fully pressed behaviour",
                Some("Fully pressing both keys registers both, for a more natural feel".into()),
                Switch::new("socd-both-bottomed-out")
                    .checked(pair.both_when_bottomed_out)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let both = *checked;
                        this.change_edited(
                            |entry| {
                                if let Behaviour::Socd(pair) = &mut entry.behaviour {
                                    pair.both_when_bottomed_out = both;
                                }
                            },
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(warning(SOCD_WARNING, cx))
    }

    pub(super) fn set_resolution(
        &mut self,
        resolution: Resolution,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_edited(
            |entry| match &mut entry.behaviour {
                Behaviour::DepthPriority(pair) => pair.both_bottomed_out = resolution,
                Behaviour::Socd(pair) => pair.resolution = resolution,
                Behaviour::DynamicKeystroke(_) | Behaviour::ModTap(_) | Behaviour::Toggle(_) => {}
            },
            window,
            cx,
        );
    }

    fn labelled_slot(
        &self,
        label: &'static str,
        slot: ActionSlot,
        action: Action,
        cx: &mut Context<Self>,
    ) -> Div {
        v_flex()
            .gap_2()
            .items_center()
            .child(div().text_sm().child(label))
            .child(self.binding_slot(slot, action, px(SLOT_SIZE), cx))
    }

    pub(super) fn binding_slot(
        &self,
        slot: ActionSlot,
        action: Action,
        size: gpui_kit::Pixels,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let caps = theme::keycaps();
        let listening = self.slot == Some(slot);
        let dragging = cx.has_active_drag();
        let accent = theme.primary;
        let filled = action != Action::None;
        div()
            .id(ElementId::Name(format!("binding-slot-{slot:?}").into()))
            .relative()
            .flex_none()
            .size(size)
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .bg(if filled { caps.top } else { theme.secondary })
            .border_1()
            .border_color(if listening || dragging {
                accent
            } else {
                theme.border
            })
            .text_xs()
            .text_color(caps.legend)
            .cursor_pointer()
            .when(filled, |this| {
                this.child(keycap_face(action, size, caps.legend, accent))
            })
            .when(listening, |this| this.child(callout(BIND_PROMPT, cx)))
            .drag_over::<Dragged<Action>>(move |style, _, _, _| {
                style.border_color(accent).bg(accent.opacity(0.25))
            })
            .on_drop(
                cx.listener(move |this, dragged: &Dragged<Action>, window, cx| {
                    this.set_slot_action(slot, *dragged.value(), window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, window, cx| {
                    this.set_slot_action(slot, Action::None, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                let next = (this.slot != Some(slot)).then_some(slot);
                this.choose_slot(next, window, cx);
            }))
    }

    fn tabs(&self, kind: Kind, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (active, hover, muted) = (
            theme.secondary,
            theme.secondary_hover,
            theme.muted_foreground,
        );
        let tabs: Vec<Stateful<Div>> = Tab::of(kind)
            .iter()
            .map(|tab| {
                let tab = *tab;
                let chosen = self.tab == tab;
                h_flex()
                    .id(SharedString::from(format!("advanced-tab-{tab:?}")))
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .rounded(theme.radius)
                    .text_sm()
                    .cursor_pointer()
                    .when(chosen, |this| this.bg(active))
                    .when(!chosen, |this| {
                        this.text_color(muted).hover(move |style| style.bg(hover))
                    })
                    .child(Icon::new(tab.icon()).small())
                    .child(tab.title())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_tab(tab, window, cx);
                    }))
            })
            .collect();
        h_flex()
            .w_full()
            .gap_1()
            .pb_2()
            .border_b_1()
            .border_color(theme.border)
            .children(tabs)
    }

    fn tab_content(&self, entry: &advanced::Key, kind: Kind, cx: &mut Context<Self>) -> AnyElement {
        match self.tab {
            Tab::Bindings => self.picker.clone().into_any_element(),
            Tab::Performance => self.performance(kind, cx).into_any_element(),
            Tab::Tester => self.tester_panes(entry, cx).into_any_element(),
            Tab::Settings => self.advanced_settings(entry, cx).into_any_element(),
        }
    }

    fn performance(&self, kind: Kind, cx: &mut Context<Self>) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        let theme = cx.theme();
        let split = settings.rapid_trigger.release.is_some();
        let enabled = settings.rapid_trigger_enabled;
        let press = self.press_sensitivity.read(cx).value().end();
        let release = self.release_sensitivity.read(cx).value().end();
        let actuation = self.actuation.read(cx).value().end();
        let sensitivity = if split {
            v_flex()
                .gap_3()
                .child(panel::field(
                    "Press sensitivity",
                    slider_with_value(Slider::new(&self.press_sensitivity), millimetres(press), cx),
                    cx,
                ))
                .child(panel::field(
                    "Release sensitivity",
                    slider_with_value(
                        Slider::new(&self.release_sensitivity),
                        millimetres(release),
                        cx,
                    ),
                    cx,
                ))
        } else {
            v_flex().child(panel::field(
                "Sensitivity",
                slider_with_value(Slider::new(&self.press_sensitivity), millimetres(press), cx),
                cx,
            ))
        };
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(PROFILE_WIDE),
            )
            .child(panel::toggle(
                "Rapid trigger",
                Some(
                    format!(
                        "Makes {} more accurate by only comparing keys you're actively pressing",
                        kind.short_title()
                    )
                    .into(),
                ),
                Switch::new("advanced-rapid-trigger")
                    .checked(enabled)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let enabled = *checked;
                        this.change_settings(
                            |settings, _, _| settings.rapid_trigger_enabled = enabled,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(panel::toggle(
                "Split sensitivity",
                Some("Separate distances for pressing and releasing".into()),
                Switch::new("advanced-split-sensitivity")
                    .checked(split)
                    .disabled(!enabled)
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let split = *checked;
                        this.change_settings(
                            |settings, _, _| {
                                settings.rapid_trigger.release =
                                    split.then_some(settings.rapid_trigger.press);
                            },
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .when(enabled, |this| this.child(sensitivity))
            .child(panel::field(
                "Actuation point",
                slider_with_value(Slider::new(&self.actuation), millimetres(actuation), cx),
                cx,
            ))
    }

    fn advanced_settings(&self, entry: &advanced::Key, cx: &mut Context<Self>) -> Div {
        let tap_duration = self.settings.map(|settings| settings.tap_duration);
        let ticks: Vec<Stateful<Div>> = TICK_RATES
            .iter()
            .map(|(value, title, description)| {
                let value = *value;
                let choice = Choice {
                    value,
                    title: (*title).to_owned(),
                    description: (*description).to_owned(),
                };
                radio_card(
                    SharedString::from(format!("tick-{value:?}")),
                    &choice,
                    tap_duration == Some(value),
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.change_settings(
                        |settings, _, _| settings.tap_duration = value,
                        window,
                        cx,
                    );
                }))
            })
            .collect();
        let tick_rate = v_flex()
            .gap_3()
            .child(section_title(
                "Game-input tick rate",
                "If a game misses inputs, hold taps for 60 or 30 ticks",
                cx,
            ))
            .child(v_flex().gap_2().children(ticks))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(Icon::new(Catalog::Info).xsmall())
                    .child("The tick rate applies to the whole profile")
                    .id("tick-rate-help")
                    .tooltip(|window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(TICK_RATE_HELP).build(window, cx)
                    }),
            );
        let Behaviour::ModTap(mod_tap) = entry.behaviour else {
            return v_flex().child(tick_rate);
        };
        v_flex()
            .gap_6()
            .child(tick_rate)
            .child(Self::just_in_time(mod_tap, cx))
            .child(self.hold_duration(cx))
    }

    fn just_in_time(mod_tap: ModTap, cx: &mut Context<Self>) -> Div {
        let cards: Vec<Stateful<Div>> = kind::interrupts()
            .into_iter()
            .map(|choice| {
                let value = choice.value;
                radio_card(
                    SharedString::from(format!("jit-{value:?}")),
                    &choice,
                    value == mod_tap.interrupt,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_interrupt(value, window, cx);
                }))
            })
            .collect();
        v_flex()
            .gap_3()
            .child(section_title(
                "Just-in-time behaviour",
                "Which action fires when you press another key before the hold duration",
                cx,
            ))
            .child(v_flex().gap_2().children(cards))
    }

    fn hold_duration(&self, cx: &mut Context<Self>) -> Div {
        let milliseconds = self.hold_time.read(cx).value().end();
        v_flex()
            .gap_3()
            .child(section_title(
                "Hold duration",
                "How long the key must be held for its hold action",
                cx,
            ))
            .child(slider_with_value(
                Slider::new(&self.hold_time),
                format!("{milliseconds:.0} ms"),
                cx,
            ))
    }

    fn set_interrupt(&mut self, interrupt: Interrupt, window: &mut Window, cx: &mut Context<Self>) {
        self.change_edited(
            |entry| {
                if let Behaviour::ModTap(mod_tap) = &mut entry.behaviour {
                    mod_tap.interrupt = interrupt;
                }
            },
            window,
            cx,
        );
    }
}

const TICK_RATES: [(TapDuration, &str, &str); 3] = [
    (
        TapDuration::OneMillisecond,
        "No tick rate",
        "Taps last 1 ms, the fastest",
    ),
    (
        TapDuration::TwentyMilliseconds,
        "60 tick rate",
        "Taps last 20 ms, long enough for 60 frames per second",
    ),
    (
        TapDuration::FortyMilliseconds,
        "30 tick rate",
        "Taps last 40 ms, long enough for 30 frames per second",
    ),
];

pub(super) fn radio_card<T>(
    id: SharedString,
    choice: &Choice<T>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    h_flex()
        .id(id)
        .w_full()
        .px_4()
        .py_3()
        .gap_3()
        .items_center()
        .rounded(theme.radius)
        .bg(if selected {
            theme.secondary_hover
        } else {
            theme.secondary
        })
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(radio(
            selected,
            theme.primary,
            theme.muted_foreground,
            theme.primary_foreground,
        ))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child(SharedString::from(choice.title.clone())),
                )
                .when(!choice.description.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(choice.description.clone())),
                    )
                }),
        )
}

fn radio(selected: bool, accent: Hsla, muted: Hsla, dot: Hsla) -> Div {
    div()
        .flex_none()
        .size(px(RADIO_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .map(|this| {
            if selected {
                this.bg(accent)
                    .child(div().size(px(RADIO_SIZE / 3.0)).rounded_full().bg(dot))
            } else {
                this.border_1().border_color(muted)
            }
        })
}

pub(super) fn section_title(title: &'static str, description: &'static str, cx: &App) -> Div {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(description),
        )
}

pub(super) fn callout(text: &'static str, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .absolute()
        .top(-px(CALLOUT_OFFSET))
        .left_0()
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .bg(theme.popover)
        .border_1()
        .border_color(theme.primary)
        .text_sm()
        .text_color(theme.foreground)
        .whitespace_nowrap()
        .child(text)
}

pub(super) fn slider_with_value(slider: Slider, value: String, cx: &App) -> Div {
    h_flex()
        .w_full()
        .gap_4()
        .items_center()
        .child(div().flex_1().min_w_0().child(slider))
        .child(
            div()
                .w(px(VALUE_WIDTH))
                .px_2()
                .py_1()
                .rounded(px(6.0))
                .bg(cx.theme().background)
                .text_right()
                .text_sm()
                .child(SharedString::from(value)),
        )
}

pub(super) fn millimetres(value: f32) -> String {
    format!("{value:.2} mm")
}

fn resolution_dropdown<const COUNT: usize>(
    id: &'static str,
    choices: [Choice<Resolution>; COUNT],
    current: Resolution,
    cx: &mut Context<AdvancedPage>,
) -> Dropdown {
    let page = cx.entity().downgrade();
    let selected = choices.iter().position(|choice| choice.value == current);
    let values: Vec<Resolution> = choices.iter().map(|choice| choice.value).collect();
    Dropdown::new(id)
        .items(choices.into_iter().map(|choice| Item::new(choice.title)))
        .selected(selected)
        .on_select(move |index, window, cx| {
            let Some(value) = values.get(index).copied() else {
                return;
            };
            page.update(cx, |page, cx| page.set_resolution(value, window, cx))
                .ok();
        })
}
