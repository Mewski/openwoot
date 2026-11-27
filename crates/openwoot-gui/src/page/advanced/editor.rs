use gpui_kit::{
    AnyElement, App, Context, Div, ElementId, Focusable as _, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, TextAlign, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, h_flex, input::Input, tooltip::Tooltip, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, DepthPriority, Interrupt, ModTap, Resolution, Socd},
    keys::{Action, TapDuration},
};

use super::{
    ActionSlot, AdvancedPage, Kind, Tab,
    creation::{CARD_MIN_HEIGHT, CARD_RADIUS},
    kind::{self, Choice},
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::Dragged,
    page::{
        performance::{Span, art_slot, distance_block, switch},
        remap::binding,
    },
    panel,
    slider::Slider,
    store::Keyboard,
    switch::Switch,
    theme,
};

const BODY_PADDING_X: f32 = 24.0;
const COLUMN_TOP: f32 = 24.0;
const COLUMN_BOTTOM: f32 = 32.0;
const COLUMN_GAP: f32 = 24.0;
const DIVIDER_OPACITY: f32 = 0.6;
const NARROW_COLUMN: f32 = 350.0;
const WIDE_COLUMN: f32 = 400.0;
const TIMELINE_COLUMN: f32 = 440.0;
const DESCRIPTION_WIDTH: f32 = 300.0;
const STACK_GAP: f32 = 24.0;
const SECTION_GAP: f32 = 32.0;
const SECTION_INNER_GAP: f32 = 12.0;
const SETTINGS_GAP: f32 = 20.0;
const SETTINGS_DIVIDER_MARGIN: f32 = 16.0;
const HEADING_LINE: f32 = 19.2;
const CHOICE_GAP: f32 = 8.0;
const CHOICE_PADDING: f32 = 12.0;
const CHOICE_RADIUS: f32 = 6.0;
const CHOICE_TEXT_GAP: f32 = 4.0;
const CHOICE_LINE: f32 = 24.0;
const RADIO_SIZE: f32 = 20.0;
const RADIO_DOT: f32 = 8.0;
const RADIO_GAP: f32 = 12.0;
const INFO_ICON: f32 = 20.0;
const TAB_GAP: f32 = 8.0;
const TAB_PADDING_X: f32 = 16.0;
const TAB_PADDING_Y: f32 = 8.0;
const TAB_RADIUS: f32 = 6.0;
const TAB_ICON: f32 = 20.0;
const TAB_TEXT: f32 = 12.8;
const PANEL_PADDING_Y: f32 = 16.0;
const SLOT_SIZE: f32 = 60.0;
const SLOTS_GAP: f32 = 8.0;
const SLOT_LABEL_TEXT: f32 = 12.8;
const DROP_TINT: f32 = 0.25;
const CALLOUT_HEIGHT: f32 = 39.0;
const CALLOUT_GAP: f32 = 7.0;
const CALLOUT_PADDING_X: f32 = 12.0;
const CALLOUT_RADIUS: f32 = 6.0;
const CALLOUT_TEXT: f32 = 14.0;
const CALLOUT_LINE: f32 = 21.0;
const PERFORMANCE_GAP: f32 = 24.0;
const RAPID_GAP: f32 = 20.0;
const TITLE_ROW: f32 = 24.0;
const ART_GAP: f32 = 32.0;
const VALUE_BOX_WIDTH: f32 = 48.0;
const VALUE_BOX_HEIGHT: f32 = 32.0;
const VALUE_BOX_RADIUS: f32 = 6.0;
const VALUE_GAP: f32 = 8.0;
const UNIT_TEXT: f32 = 12.8;
const SLIDER_ROW_TOP: f32 = 12.0;
const SLIDER_ROW_BOTTOM: f32 = 24.0;
const BIND_PROMPT: &str = "Press any key to bind";
const RAPPY_TITLE: &str = "Fully pressed behaviour";
const RAPPY_DESCRIPTION: &str = "Select what happens when both keys are fully pressed.";
const SOCD_TITLE: &str = "SOCD Cleaning behaviour";
const SOCD_DESCRIPTION: &str = "Select what happens when both keys are activated.";
const ALTERNATIVE_TITLE: &str = "Alternative fully pressed behaviour";
const ALTERNATIVE_DESCRIPTION: &str = "Fully pressing both keys at the same time registers your inputs as normal for a more natural feel.";
const MOD_TAP_DESCRIPTION: &str =
    "Hold the key down for the first action or tap the key for the second action.";
const TOGGLE_DESCRIPTION: &str = "Drag and drop the binding you wish to toggle.";
const RAPID_TRIGGER: &str = "Rapid Trigger";
const RAPID_TRIGGER_HELP: &str = "Changes to Rapid Trigger apply to all keys on your keyboard.";
const SPLIT: &str = "Split sensitivity";
const SPLIT_HELP: &str = "Set separate activation and deactivation distances.";
const ACTUATION: &str = "Actuation Point";
const ACTUATION_HELP: &str = "Changes to Actuation Point apply to all keys on your keyboard.";
const ACTUATION_DESCRIPTION: &str = "Set how far down you have to press the keys to activate it.";
const TICK_RATE: &str = "Game-input tick rate";
const TICK_RATE_NOTE: &str = "The tick rate is applied per profile.";
const JUST_IN_TIME: &str = "Just-in-time behaviour";
const JUST_IN_TIME_DESCRIPTION: &str =
    "Decides which function to trigger when you press another key before the Mod Tap activates.";
const HOLD_DURATION: &str = "Hold Duration";
const HOLD_DURATION_DESCRIPTION: &str =
    "Set how long you need to hold the key before its \"Hold\" key activates.";
const TICK_RATES: [(TapDuration, &str); 3] = [
    (TapDuration::OneMillisecond, "No Tick rate"),
    (TapDuration::TwentyMilliseconds, "60 Tick rate"),
    (TapDuration::FortyMilliseconds, "30 Tick rate"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Look {
    Informed,
    Plain,
    Described,
}

impl AdvancedPage {
    pub(super) fn editor_card(
        &self,
        entry: &advanced::Key,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let kind = Kind::of(&entry.behaviour);
        let entry = *entry;
        let actions = h_flex()
            .child(
                panel::ActionButton::new("advanced-delete", "Delete")
                    .icon(Catalog::Trash)
                    .on_click(cx.listener(move |_, _, window, cx| {
                        Self::confirm_delete(entry, window, cx);
                    })),
            )
            .child(
                panel::ActionButton::new("advanced-done", "Done")
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| this.overview(cx))),
            );
        let header = Self::card_header(kind, actions, cx);
        let left = v_flex()
            .flex_none()
            .w(px(column_width(kind)))
            .pt(px(COLUMN_TOP))
            .pb(px(COLUMN_BOTTOM))
            .child(self.settings_of(&entry, keyboard, cx));
        let right = v_flex()
            .flex_1()
            .min_w_0()
            .pt(px(COLUMN_TOP))
            .pb(px(COLUMN_BOTTOM))
            .child(self.tabs(kind, cx))
            .child(
                div()
                    .w_full()
                    .py(px(PANEL_PADDING_Y))
                    .child(self.tab_content(&entry, kind, keyboard, window, cx)),
            );
        v_flex()
            .w_full()
            .min_h(px(CARD_MIN_HEIGHT))
            .rounded(px(CARD_RADIUS))
            .bg(cx.theme().group_box)
            .child(header)
            .child(
                h_flex()
                    .w_full()
                    .flex_1()
                    .px(px(BODY_PADDING_X))
                    .items_stretch()
                    .gap(px(COLUMN_GAP))
                    .child(left)
                    .child(divider(cx))
                    .child(right),
            )
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
            Behaviour::ModTap(mod_tap) => v_flex()
                .gap(px(STACK_GAP))
                .child(panel::description(MOD_TAP_DESCRIPTION, cx))
                .child(
                    h_flex()
                        .w_full()
                        .gap(px(SLOTS_GAP))
                        .justify_center()
                        .child(self.labelled_slot("Hold", ActionSlot::Hold, mod_tap.hold, cx))
                        .child(self.labelled_slot("Tap", ActionSlot::Tap, mod_tap.tap, cx)),
                )
                .into_any_element(),
            Behaviour::Toggle(action) => v_flex()
                .gap(px(STACK_GAP))
                .child(panel::description(TOGGLE_DESCRIPTION, cx))
                .child(h_flex().w_full().justify_center().child(self.binding_slot(
                    ActionSlot::Toggle,
                    action,
                    px(SLOT_SIZE),
                    cx,
                )))
                .into_any_element(),
            Behaviour::DepthPriority(pair) => {
                self.rappy_settings(entry, pair, cx).into_any_element()
            }
            Behaviour::Socd(pair) => self.socd_settings(entry, pair, cx).into_any_element(),
        }
    }

    fn rappy_settings(
        &self,
        entry: &advanced::Key,
        pair: DepthPriority,
        cx: &mut Context<Self>,
    ) -> Div {
        let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
        let choices = kind::bottomed_out_resolutions(&first, &second);
        v_flex()
            .gap(px(SECTION_GAP))
            .child(
                div()
                    .max_w(px(DESCRIPTION_WIDTH))
                    .child(panel::description(Kind::DepthPriority.help(), cx)),
            )
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(heading(RAPPY_TITLE, cx))
                    .child(panel::description(RAPPY_DESCRIPTION, cx))
                    .child(resolution_dropdown(
                        "depth-bottomed-out",
                        choices,
                        pair.both_bottomed_out,
                        cx,
                    )),
            )
    }

    fn socd_settings(&self, entry: &advanced::Key, pair: Socd, cx: &mut Context<Self>) -> Div {
        let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
        let cards: Vec<Stateful<Div>> = kind::socd_resolutions(&first, &second)
            .into_iter()
            .map(|choice| {
                let value = choice.value;
                radio_card(
                    SharedString::from(format!("socd-{value:?}")),
                    &choice,
                    value == pair.resolution,
                    Look::Informed,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_resolution(value, window, cx);
                }))
            })
            .collect();
        v_flex()
            .gap(px(SECTION_GAP))
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(heading(SOCD_TITLE, cx))
                    .child(panel::description(SOCD_DESCRIPTION, cx))
                    .child(v_flex().gap(px(CHOICE_GAP)).children(cards)),
            )
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(
                        h_flex()
                            .w_full()
                            .h(px(TITLE_ROW))
                            .gap(px(CHOICE_GAP))
                            .items_center()
                            .justify_between()
                            .child(heading(ALTERNATIVE_TITLE, cx))
                            .child(
                                Switch::new("socd-both-bottomed-out")
                                    .checked(pair.both_when_bottomed_out)
                                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                                        let both = *checked;
                                        this.change_edited(
                                            |entry| {
                                                if let Behaviour::Socd(pair) = &mut entry.behaviour
                                                {
                                                    pair.both_when_bottomed_out = both;
                                                }
                                            },
                                            window,
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(panel::description(ALTERNATIVE_DESCRIPTION, cx)),
            )
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
            .gap(px(SLOTS_GAP))
            .items_center()
            .child(
                div()
                    .text_size(px(SLOT_LABEL_TEXT))
                    .line_height(px(HEADING_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::keycaps().legend)
                    .child(label),
            )
            .child(self.binding_slot(slot, action, px(SLOT_SIZE), cx))
    }

    pub(super) fn binding_slot(
        &self,
        slot: ActionSlot,
        action: Action,
        size: gpui_kit::Pixels,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let listening = self.slot == Some(slot);
        let dragging = cx.has_active_drag();
        let accent = cx.theme().primary;
        binding(action, self.legend(action, cx), size, cx)
            .id(ElementId::Name(format!("binding-slot-{slot:?}").into()))
            .relative()
            .cursor_pointer()
            .when(listening || dragging, |this| this.border_color(accent))
            .when(listening, |this| this.child(callout(BIND_PROMPT, cx)))
            .drag_over::<Dragged<Action>>(move |style, _, _, _| {
                style.border_color(accent).bg(accent.opacity(DROP_TINT))
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
        let (selected, hover, accent, muted, border) = (
            theme.secondary,
            theme.accent,
            theme.primary,
            theme.muted_foreground,
            theme.border,
        );
        let legend = theme::keycaps().legend;
        let tabs: Vec<Stateful<Div>> = Tab::of(kind)
            .iter()
            .map(|tab| {
                let tab = *tab;
                let chosen = self.tab == tab;
                h_flex()
                    .id(SharedString::from(format!("advanced-tab-{tab:?}")))
                    .px(px(TAB_PADDING_X))
                    .py(px(TAB_PADDING_Y))
                    .gap(px(TAB_GAP))
                    .items_center()
                    .rounded_t(px(TAB_RADIUS))
                    .cursor_pointer()
                    .when(chosen, |this| this.bg(selected))
                    .when(!chosen, |this| this.hover(move |style| style.bg(hover)))
                    .child(
                        Icon::new(tab.icon())
                            .size(px(TAB_ICON))
                            .text_color(if chosen { accent } else { muted }),
                    )
                    .child(
                        div()
                            .text_size(px(TAB_TEXT))
                            .line_height(px(HEADING_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(legend)
                            .child(tab.title()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose_tab(tab, window, cx);
                    }))
            })
            .collect();
        h_flex()
            .w_full()
            .gap(px(TAB_GAP))
            .border_b_1()
            .border_color(border)
            .children(tabs)
    }

    fn tab_content(
        &self,
        entry: &advanced::Key,
        kind: Kind,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match self.tab {
            Tab::Bindings => self.picker.clone().into_any_element(),
            Tab::Performance => self
                .performance(kind, entry, keyboard, cx)
                .into_any_element(),
            Tab::Tester => self.tester_panes(entry, cx).into_any_element(),
            Tab::Settings => self.advanced_settings(entry, window, cx).into_any_element(),
        }
    }

    fn performance(
        &self,
        kind: Kind,
        entry: &advanced::Key,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        let split = settings.rapid_trigger.release.is_some();
        let enabled = settings.rapid_trigger_enabled;
        let rapid_trigger = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(RAPID_GAP))
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(
                        h_flex()
                            .w_full()
                            .h(px(TITLE_ROW))
                            .items_center()
                            .justify_between()
                            .child(titled(RAPID_TRIGGER, RAPID_TRIGGER_HELP, cx))
                            .child(
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
                            ),
                    )
                    .child(panel::description(
                        format!(
                            "Makes {} more accurate by only comparing keys you're actively pressing.",
                            kind.short_title()
                        ),
                        cx,
                    )),
            )
            .child(panel::toggle(
                SPLIT,
                Some(SPLIT_HELP.into()),
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
            .map(|this| {
                if split {
                    this.child(distance_block("Key press - activate", &self.press, cx))
                        .child(distance_block("Key release - deactivate", &self.release, cx))
                } else {
                    this.child(distance_block("Sensitivity", &self.press, cx))
                }
            });
        let key = usize::from(entry.key.get());
        let depth = Span::actuation(keyboard.travel_mm(key))
            .fraction(keyboard.distance_mm(key, settings.actuation));
        let actuation = v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(SECTION_INNER_GAP))
            .child(titled(ACTUATION, ACTUATION_HELP, cx))
            .child(panel::description(ACTUATION_DESCRIPTION, cx))
            .child(
                h_flex()
                    .gap(px(ART_GAP))
                    .items_center()
                    .child(art_slot(switch(depth, cx.theme().success, cx)))
                    .child(self.actuation.clone()),
            );
        h_flex()
            .w_full()
            .items_stretch()
            .gap(px(PERFORMANCE_GAP))
            .child(rapid_trigger)
            .child(divider(cx))
            .child(actuation)
    }

    fn advanced_settings(
        &self,
        entry: &advanced::Key,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let tap_duration = self.settings.map(|settings| settings.tap_duration);
        let ticks: Vec<Stateful<Div>> = TICK_RATES
            .iter()
            .map(|(value, title)| {
                let value = *value;
                let choice = Choice {
                    value,
                    title: (*title).to_owned(),
                    description: String::new(),
                };
                radio_card(
                    SharedString::from(format!("tick-{value:?}")),
                    &choice,
                    tap_duration == Some(value),
                    Look::Plain,
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
        let kind = Kind::of(&entry.behaviour);
        let tick_rate = v_flex()
            .gap(px(SETTINGS_GAP))
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(heading(TICK_RATE, cx))
                    .child(panel::description(
                        format!(
                            "If you're experiencing missed inputs with {}, try lowering the tick rate to 60 or 30.",
                            kind.abbreviation()
                        ),
                        cx,
                    )),
            )
            .child(v_flex().gap(px(CHOICE_GAP)).children(ticks))
            .child(panel::info_compact(TICK_RATE_NOTE, cx));
        let Behaviour::ModTap(mod_tap) = entry.behaviour else {
            return v_flex().child(tick_rate);
        };
        v_flex()
            .child(tick_rate)
            .child(settings_divider(cx))
            .child(Self::just_in_time(mod_tap, cx))
            .child(settings_divider(cx))
            .child(self.hold_duration(window, cx))
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
                    Look::Described,
                    cx,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_interrupt(value, window, cx);
                }))
            })
            .collect();
        v_flex()
            .gap(px(SETTINGS_GAP))
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(heading(JUST_IN_TIME, cx))
                    .child(panel::description(JUST_IN_TIME_DESCRIPTION, cx)),
            )
            .child(v_flex().gap(px(CHOICE_GAP)).children(cards))
    }

    fn hold_duration(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let (input, border, focus, foreground) = (
            theme::palette().input,
            theme.border,
            theme.primary,
            theme.foreground,
        );
        let focused = self.hold_input.focus_handle(cx).is_focused(window);
        v_flex()
            .gap(px(SETTINGS_GAP))
            .child(
                v_flex()
                    .gap(px(SECTION_INNER_GAP))
                    .child(heading(HOLD_DURATION, cx))
                    .child(panel::description(HOLD_DURATION_DESCRIPTION, cx)),
            )
            .child(
                h_flex()
                    .w_full()
                    .pt(px(SLIDER_ROW_TOP))
                    .pb(px(SLIDER_ROW_BOTTOM))
                    .gap(px(VALUE_GAP))
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Slider::new(&self.hold_time)))
                    .child(
                        div()
                            .flex_none()
                            .w(px(VALUE_BOX_WIDTH))
                            .h(px(VALUE_BOX_HEIGHT))
                            .flex()
                            .items_center()
                            .rounded(px(VALUE_BOX_RADIUS))
                            .bg(input)
                            .border_1()
                            .border_color(if focused { focus } else { border })
                            .text_color(foreground)
                            .child(
                                Input::new(&self.hold_input)
                                    .appearance(false)
                                    .xsmall()
                                    .w_full()
                                    .text_align(TextAlign::Center),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(UNIT_TEXT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground)
                            .child("ms"),
                    ),
            )
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

const fn column_width(kind: Kind) -> f32 {
    match kind {
        Kind::DynamicKeystroke => TIMELINE_COLUMN,
        Kind::ModTap | Kind::Toggle => WIDE_COLUMN,
        Kind::DepthPriority | Kind::Socd => NARROW_COLUMN,
    }
}

pub(super) fn radio_card<T>(
    id: SharedString,
    choice: &Choice<T>,
    selected: bool,
    look: Look,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let (fill, hover, text, muted) = (
        if selected {
            theme.secondary_hover
        } else {
            theme.accent
        },
        if selected {
            theme.secondary_hover
        } else {
            theme.secondary
        },
        theme.group_box_foreground,
        theme.muted_foreground,
    );
    let title = div()
        .line_height(px(if look == Look::Informed {
            CHOICE_LINE
        } else {
            HEADING_LINE
        }))
        .font_weight(match look {
            Look::Informed => FontWeight::MEDIUM,
            Look::Plain => FontWeight::NORMAL,
            Look::Described => FontWeight::SEMIBOLD,
        })
        .child(SharedString::from(choice.title.clone()));
    let description = SharedString::from(choice.description.clone());
    let info = SharedString::from(format!("{id}-info"));
    h_flex()
        .id(id)
        .w_full()
        .p(px(CHOICE_PADDING))
        .gap(px(RADIO_GAP))
        .items_center()
        .rounded(px(CHOICE_RADIUS))
        .bg(fill)
        .text_base()
        .text_color(text)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(radio(selected, cx))
        .map(|this| match look {
            Look::Described => this.child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(CHOICE_TEXT_GAP))
                    .child(title)
                    .child(div().line_height(px(HEADING_LINE)).child(description)),
            ),
            Look::Plain => this.child(title.flex_1().min_w_0()),
            Look::Informed => this.child(title.flex_1().min_w_0()).child(
                div()
                    .id(info)
                    .flex_none()
                    .text_color(muted)
                    .child(Icon::new(Catalog::Info).size(px(INFO_ICON)))
                    .tooltip(move |window, cx| Tooltip::new(description.clone()).build(window, cx)),
            ),
        })
}

fn radio(selected: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .size(px(RADIO_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(if selected {
            theme.primary
        } else {
            theme.secondary
        })
        .when(selected, |this| {
            this.child(
                div()
                    .size(px(RADIO_DOT))
                    .rounded_full()
                    .bg(theme.primary_foreground),
            )
        })
}

pub(super) fn heading(title: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_base()
        .line_height(px(HEADING_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().foreground)
        .child(title.into())
}

fn titled(title: &'static str, help: &'static str, cx: &App) -> Div {
    let owner = SharedString::from(title);
    h_flex()
        .gap(px(CHOICE_GAP))
        .items_center()
        .child(heading(title, cx))
        .child(panel::help(&owner, help.into(), cx))
}

pub(super) fn divider(cx: &App) -> Div {
    div()
        .flex_none()
        .w(px(1.0))
        .bg(cx.theme().border)
        .opacity(DIVIDER_OPACITY)
}

fn settings_divider(cx: &App) -> Div {
    div()
        .w_full()
        .my(px(SETTINGS_DIVIDER_MARGIN))
        .h(px(1.0))
        .bg(cx.theme().border)
        .opacity(DIVIDER_OPACITY)
}

pub(super) fn callout(text: &'static str, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .absolute()
        .left_0()
        .right_0()
        .top(-px(CALLOUT_HEIGHT + CALLOUT_GAP))
        .h(px(CALLOUT_HEIGHT))
        .justify_center()
        .child(
            div()
                .flex_none()
                .h_full()
                .px(px(CALLOUT_PADDING_X))
                .flex()
                .items_center()
                .rounded(px(CALLOUT_RADIUS))
                .bg(theme::palette().tooltip)
                .border_1()
                .border_color(theme.border)
                .shadow_md()
                .whitespace_nowrap()
                .text_size(px(CALLOUT_TEXT))
                .line_height(px(CALLOUT_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(text),
        )
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
