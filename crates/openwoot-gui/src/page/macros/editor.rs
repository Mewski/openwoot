use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, FontWeight, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, input::Input, spinner::Spinner,
        tooltip::Tooltip, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    keys::Action,
    macros::{MAX_ENCODED_LEN, MAX_STEPS, Macro, Mode, Step},
};

use super::{
    Field, KeyTarget, MODES, MacrosPage, mode_detail, mode_title,
    recording::Timing,
    step::{self, Kind, Part},
};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::Dragged,
    page::{profiles::count_chip, remap::chip},
    panel,
    store::Keyboard,
    theme,
};

const PLAYBACK_WIDTH: f32 = 240.0;
const TIMING_WIDTH: f32 = 168.0;
const STEP_HEIGHT: f32 = 44.0;
const STEP_RADIUS: f32 = 6.0;
const GRIP: f32 = 16.0;
const NUMBER_WIDTH: f32 = 28.0;
const KIND_WIDTH: f32 = 112.0;
const KIND_HEIGHT: f32 = 30.0;
const KIND_ICON: f32 = 16.0;
const KIND_MENU_WIDTH: f32 = 200.0;
const ADD_MENU_WIDTH: f32 = 240.0;
const CHEVRON: f32 = 14.0;
const VALUE_HEIGHT: f32 = 30.0;
const VALUE_RADIUS: f32 = 6.0;
const VALUE_TEXT: f32 = 14.0;
const DELAY_WIDTH: f32 = 84.0;
const NUMBER_BOX_WIDTH: f32 = 72.0;
const KEY_RADIUS: f32 = 10.0;
const DRAG_WIDTH: f32 = 320.0;
const RECORDING_DOT: f32 = 8.0;
const PREVIEW_OPACITY: f32 = 0.7;
const NO_SELECTION: &str = "Create a macro or select one from your library to edit its steps.";
const NO_STEPS: &str = "Add steps with the button below, click keys in the list or record them.";
const RECORDING: &str = "Recording, press keys on your keyboard";
const LOADING: &str = "Loading the macro";

#[derive(Debug, Clone)]
struct StepDrag {
    index: usize,
    kind: Kind,
    label: SharedString,
}

impl MacrosPage {
    pub(super) fn editor_card(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let Some(index) = self.selected else {
            return panel::section("No Macro Selected", Some(NO_SELECTION.into()), cx);
        };
        let Some(value) = self.loaded().cloned() else {
            let title = keyboard.macros.name(index).map_or_else(
                || SharedString::from("Macro"),
                |name| name.to_owned().into(),
            );
            return panel::section(title, None, cx).child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Spinner::new().small())
                    .child(LOADING),
            );
        };
        let recording = self.recorder.is_some();
        panel::section_with(
            SharedString::from(value.name.as_str().to_owned()),
            None,
            self.record_controls(cx),
            cx,
        )
        .child(
            h_flex()
                .w_full()
                .gap_4()
                .items_start()
                .child(div().flex_1().min_w_0().child(panel::field(
                    "Name",
                    panel::text_field(&self.name, "Macro name", window, cx),
                    cx,
                )))
                .child(div().flex_none().w(px(PLAYBACK_WIDTH)).child(panel::field(
                    "Playback",
                    Self::mode_menu(value.mode, recording, cx),
                    cx,
                ))),
        )
        .child(panel::note(mode_detail(value.mode), cx))
        .child(self.steps_header(&value, cx))
        .child(self.step_list(&value, keyboard, cx))
        .child(self.add_bar(&value, cx))
    }

    pub(super) fn keys_card(&self, cx: &mut Context<Self>) -> Option<Div> {
        self.loaded()?;
        if self.recorder.is_some() {
            return None;
        }
        let (title, text) = match self.target {
            KeyTarget::Append => (
                "Add Keys",
                match self.chosen {
                    Some(index) => format!(
                        "Click a key to add a tap after step {}, or drag it onto a step.",
                        index + 1
                    ),
                    None => {
                        "Click a key to add a tap at the end, or drag it onto a step.".to_owned()
                    }
                },
            ),
            KeyTarget::Step(index) => (
                "Change Key",
                format!("Press a key or pick one below for step {}.", index + 1),
            ),
            KeyTarget::New(kind) => (
                "Choose a Key",
                format!(
                    "Press a key or pick one below for the new {}.",
                    kind.title().to_lowercase()
                ),
            ),
        };
        let card = if self.target == KeyTarget::Append {
            panel::section(title, Some(text.into()), cx)
        } else {
            panel::section_with(
                title,
                Some(text.into()),
                panel::ActionButton::new("macro-key-cancel", "Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.retarget(KeyTarget::Append, cx))),
                cx,
            )
        };
        Some(card.child(self.picker.clone()))
    }

    fn record_controls(&self, cx: &mut Context<Self>) -> Div {
        let page = cx.entity().downgrade();
        let recording = self.recorder.is_some();
        let timing = Dropdown::new("macro-timing")
            .small()
            .items(Timing::ALL.map(|timing| Item::new(timing.label())))
            .selected(Timing::ALL.iter().position(|timing| *timing == self.timing))
            .on_select(move |position, _, cx| {
                if let Some(timing) = Timing::ALL.get(position).copied() {
                    page.update(cx, |page, cx| page.set_timing(timing, cx)).ok();
                }
            });
        let button = if recording {
            panel::ActionButton::new("macro-stop", "Stop")
                .danger()
                .icon(Icon::new(Catalog::Square))
                .on_click(cx.listener(|this, _, window, cx| this.stop_recording(window, cx)))
        } else {
            panel::ActionButton::new("macro-record", "Record")
                .primary()
                .icon(Icon::new(Catalog::CircleDot))
                .tooltip("Record keys from your keyboard")
                .on_click(cx.listener(|this, _, window, cx| this.start_recording(window, cx)))
        };
        h_flex()
            .gap_2()
            .items_center()
            .child(div().w(px(TIMING_WIDTH)).child(timing))
            .child(button)
    }

    fn mode_menu(mode: Mode, recording: bool, cx: &mut Context<Self>) -> Dropdown {
        let page = cx.entity().downgrade();
        Dropdown::new("macro-mode")
            .items(MODES.map(|mode| Item::new(mode_title(mode))))
            .selected(MODES.iter().position(|known| *known == mode))
            .disabled(recording)
            .on_select(move |position, window, cx| {
                if let Some(mode) = MODES.get(position).copied() {
                    page.update(cx, |page, cx| page.set_mode(mode, window, cx))
                        .ok();
                }
            })
    }

    fn steps_header(&self, value: &Macro, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let recorded = self.recorded().map_or(0, |(_, steps)| steps.len());
        let bytes = value.encoded_len();
        let (muted, danger) = (theme.muted_foreground, theme.danger);
        let status = if self.recorder.is_some() {
            h_flex()
                .gap_2()
                .items_center()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(danger)
                .child(div().size(px(RECORDING_DOT)).rounded_full().bg(danger))
                .child(RECORDING)
        } else {
            div()
                .text_xs()
                .text_color(if bytes > MAX_ENCODED_LEN {
                    danger
                } else {
                    muted
                })
                .child(format!("{bytes} of {MAX_ENCODED_LEN} bytes"))
        };
        h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(panel::label("Steps", cx))
                    .child(count_chip(value.steps.len() + recorded, MAX_STEPS, cx)),
            )
            .child(status)
    }

    fn step_list(
        &self,
        value: &Macro,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let recorded = self.recorded();
        let preview = |at: usize, cx: &mut Context<Self>| -> Vec<AnyElement> {
            recorded
                .iter()
                .filter(|(start, _)| *start == at)
                .flat_map(|(start, steps)| {
                    steps
                        .iter()
                        .enumerate()
                        .map(move |(offset, step)| (start + offset + 1, step))
                })
                .map(|(number, step)| preview_row(number, step, keyboard, cx).into_any_element())
                .collect()
        };
        let shift = |position: usize| {
            recorded
                .as_ref()
                .filter(|(at, _)| position >= *at)
                .map_or(0, |(_, steps)| steps.len())
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        for (position, step) in value.steps.iter().enumerate() {
            rows.extend(preview(position, cx));
            rows.push(
                self.step_row(position, position + 1 + shift(position), step, keyboard, cx)
                    .into_any_element(),
            );
        }
        rows.extend(preview(value.steps.len(), cx));
        let theme = cx.theme();
        let (muted, accent) = (theme.muted_foreground, theme.primary);
        let empty = rows.is_empty();
        v_flex()
            .id("macro-steps")
            .w_full()
            .gap_1()
            .rounded(px(STEP_RADIUS))
            .children(rows)
            .when(empty, |this| {
                this.child(
                    div()
                        .w_full()
                        .py_6()
                        .text_center()
                        .text_sm()
                        .text_color(muted)
                        .child(NO_STEPS),
                )
            })
            .when(self.recorder.is_none(), |this| {
                this.drag_over::<Dragged<Action>>(move |style, _, _, _| {
                    style.bg(accent.opacity(0.08))
                })
                .on_drop(cx.listener(
                    |this, dragged: &Dragged<Action>, window, cx| {
                        this.append_dropped(*dragged.value(), window, cx);
                    },
                ))
            })
    }

    fn step_row(
        &self,
        position: usize,
        number: usize,
        step: &Step,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let kind = Kind::of(step);
        let recording = self.recorder.is_some();
        let chosen = self.chosen == Some(position) && !recording;
        let theme = cx.theme();
        let (fill, border, accent, muted) = (
            theme.accent,
            if chosen {
                theme.primary
            } else {
                theme.transparent
            },
            theme.primary,
            theme.muted_foreground,
        );
        let drag = StepDrag {
            index: position,
            kind,
            label: summary(step, keyboard),
        };
        let values: Vec<AnyElement> = match step::action(step) {
            Some(action) => vec![
                self.key_slot(position, action, keyboard, cx)
                    .into_any_element(),
            ],
            None => kind
                .parts()
                .iter()
                .map(|part| {
                    self.value_box(
                        Field {
                            step: position,
                            part: *part,
                        },
                        step,
                        cx,
                    )
                })
                .collect(),
        };
        let row = h_flex()
            .id(SharedString::from(format!("macro-step-{position}")))
            .w_full()
            .min_h(px(STEP_HEIGHT))
            .pl_1()
            .pr_1p5()
            .gap_2()
            .items_center()
            .rounded(px(STEP_RADIUS))
            .border_1()
            .border_color(border)
            .bg(fill)
            .child(
                div()
                    .id(SharedString::from(format!("macro-grip-{position}")))
                    .flex_none()
                    .text_color(muted)
                    .when(!recording, |this| {
                        this.cursor_grab()
                            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                    })
                    .child(Icon::new(Catalog::GripVertical).size(px(GRIP))),
            )
            .child(number_cell(number, muted))
            .child(Self::kind_cell(position, kind, recording, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .children(values),
            );
        if recording {
            return row;
        }
        row.child(
            panel::ActionButton::icon_only(
                SharedString::from(format!("macro-remove-{position}")),
                Icon::new(Catalog::Trash),
            )
            .tooltip("Remove step")
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.remove_step(position, window, cx);
            })),
        )
        .cursor_pointer()
        .drag_over::<StepDrag>(move |style, _, _, _| style.border_color(accent))
        .on_drop(cx.listener(move |this, drag: &StepDrag, window, cx| {
            this.move_step(drag.index, position, window, cx);
        }))
        .on_click(cx.listener(move |this, _, _, cx| this.choose_step(position, cx)))
    }

    fn kind_cell(
        position: usize,
        kind: Kind,
        recording: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !kind.is_key() || recording {
            return kind_face(kind, false, false, cx).into_any_element();
        }
        let page = cx.entity().downgrade();
        div()
            .id(SharedString::from(format!("macro-kind-{position}")))
            .flex_none()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Dropdown::new(SharedString::from(format!("macro-kind-menu-{position}")))
                    .trigger(move |open, cx| kind_face(kind, open, true, cx).into_any_element())
                    .menu_width(px(KIND_MENU_WIDTH))
                    .items(Kind::KEYS.map(|kind| Item::new(kind.title()).icon(kind.icon())))
                    .on_select(move |chosen, window, cx| {
                        if let Some(kind) = Kind::KEYS.get(chosen).copied() {
                            page.update(cx, |page, cx| page.set_kind(position, kind, window, cx))
                                .ok();
                        }
                    }),
            )
            .into_any_element()
    }

    fn key_slot(
        &self,
        position: usize,
        action: Action,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let listening = self.target == KeyTarget::Step(position);
        let recording = self.recorder.is_some();
        let theme = cx.theme();
        let (accent, muted) = (theme.primary, theme.muted_foreground);
        let slot = h_flex()
            .id(SharedString::from(format!("macro-key-{position}")))
            .flex_none()
            .gap_2()
            .items_center()
            .p_0p5()
            .rounded(px(KEY_RADIUS))
            .border_1()
            .border_color(if listening { accent } else { theme.transparent })
            .child(chip(action, keyboard.legend(action), cx))
            .when(listening, |this| {
                this.child(
                    div()
                        .pr_2()
                        .text_xs()
                        .text_color(muted)
                        .child("Press a key"),
                )
            });
        if recording {
            return slot;
        }
        slot.cursor_pointer()
            .tooltip(|window, cx| Tooltip::new("Change the key").build(window, cx))
            .drag_over::<Dragged<Action>>(move |style, _, _, _| style.border_color(accent))
            .on_drop(
                cx.listener(move |this, dragged: &Dragged<Action>, window, cx| {
                    this.set_action(position, *dragged.value(), window, cx);
                }),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.choose_key(position, window, cx);
            }))
    }

    fn value_box(&self, field: Field, step: &Step, cx: &mut Context<Self>) -> AnyElement {
        let editing = self.field == Some(field);
        let recording = self.recorder.is_some();
        let theme = cx.theme();
        let (muted, foreground, accent, border, hover) = (
            theme.muted_foreground,
            theme.group_box_foreground,
            theme.primary,
            theme.border,
            theme.secondary_hover,
        );
        let input = theme::palette().input;
        let shown = field.part.shown(step);
        let empty = shown.is_empty();
        let content = if editing {
            div()
                .w_full()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.cancel_field(cx);
                    }
                }))
                .child(
                    Input::new(&self.value)
                        .appearance(false)
                        .px_0()
                        .text_size(px(VALUE_TEXT)),
                )
                .into_any_element()
        } else {
            div()
                .w_full()
                .truncate()
                .text_size(px(VALUE_TEXT))
                .text_color(if empty { muted } else { foreground })
                .child(if empty {
                    SharedString::from("Empty")
                } else {
                    SharedString::from(shown)
                })
                .into_any_element()
        };
        let help = field.part.help();
        let frame = h_flex()
            .id(SharedString::from(format!(
                "macro-value-{}-{:?}",
                field.step, field.part
            )))
            .h(px(VALUE_HEIGHT))
            .px_2()
            .items_center()
            .rounded(px(VALUE_RADIUS))
            .bg(input)
            .border_1()
            .border_color(if editing { accent } else { border })
            .map(|this| match field.part {
                Part::Text => this.flex_1().min_w_0(),
                Part::Delay => this.flex_none().w(px(DELAY_WIDTH)),
                Part::MoveX | Part::MoveY | Part::Vertical | Part::Horizontal => {
                    this.flex_none().w(px(NUMBER_BOX_WIDTH))
                }
            })
            .child(content)
            .tooltip(move |window, cx| Tooltip::new(help).build(window, cx))
            .when(!editing && !recording, |this| {
                this.cursor_text()
                    .hover(move |style| style.border_color(hover))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.begin_field(field, window, cx);
                    }))
            });
        h_flex()
            .gap_1p5()
            .items_center()
            .when(field.part == Part::Text, |this| this.flex_1().min_w_0())
            .children(field.part.prefix().map(|prefix| unit(prefix, muted)))
            .child(frame)
            .children(field.part.unit().map(|text| unit(text, muted)))
            .into_any_element()
    }

    fn add_bar(&self, value: &Macro, cx: &mut Context<Self>) -> Div {
        let page = cx.entity().downgrade();
        let blocked = self.recorder.is_some() || value.steps.len() >= MAX_STEPS;
        let muted = cx.theme().muted_foreground;
        let place = match (self.target, self.chosen) {
            (KeyTarget::New(_), _) => "Press a key or pick one below".to_owned(),
            (_, Some(chosen)) if chosen + 1 < value.steps.len() => {
                format!("New steps go after step {}", chosen + 1)
            }
            _ => "New steps go at the end".to_owned(),
        };
        h_flex()
            .w_full()
            .gap_3()
            .items_center()
            .child(
                Dropdown::new("macro-add-step")
                    .disabled(blocked)
                    .trigger(move |open, _| {
                        panel::ActionButton::new("macro-add-step-button", "Add Step")
                            .icon(Icon::new(IconName::Plus))
                            .selected(open)
                            .disabled(blocked)
                            .into_any_element()
                    })
                    .menu_width(px(ADD_MENU_WIDTH))
                    .items(Kind::ALL.map(|kind| Item::new(kind.title()).icon(kind.icon())))
                    .on_select(move |position, window, cx| {
                        if let Some(kind) = Kind::ALL.get(position).copied() {
                            page.update(cx, |page, cx| page.add_step(kind, window, cx))
                                .ok();
                        }
                    }),
            )
            .child(div().text_xs().text_color(muted).child(place))
    }
}

impl Render for StepDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .w(px(DRAG_WIDTH))
            .h(px(STEP_HEIGHT))
            .px_3()
            .gap_2()
            .items_center()
            .rounded(px(STEP_RADIUS))
            .border_1()
            .border_color(theme.primary)
            .bg(theme.accent)
            .shadow_lg()
            .child(
                Icon::new(self.kind.icon())
                    .size(px(KIND_ICON))
                    .text_color(theme.muted_foreground),
            )
            .child(
                div()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(self.label.clone()),
            )
    }
}

fn kind_face(kind: Kind, open: bool, choosable: bool, cx: &App) -> Div {
    let theme = cx.theme();
    let (hover, muted) = (theme.secondary, theme.muted_foreground);
    h_flex()
        .flex_none()
        .w(px(KIND_WIDTH))
        .h(px(KIND_HEIGHT))
        .px_2()
        .gap_1p5()
        .items_center()
        .rounded(px(VALUE_RADIUS))
        .when(open, |this| this.bg(hover))
        .when(choosable, |this| {
            this.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .child(Icon::new(kind.icon()).size(px(KIND_ICON)).text_color(muted))
        .child(
            div()
                .flex_1()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(kind.label()),
        )
        .when(choosable, |this| {
            this.child(
                Icon::new(Catalog::ChevronDown)
                    .size(px(CHEVRON))
                    .text_color(muted),
            )
        })
}

fn preview_row(number: usize, step: &Step, keyboard: &Keyboard, cx: &App) -> Div {
    let theme = cx.theme();
    let kind = Kind::of(step);
    let value = match step::action(step) {
        Some(action) => chip(action, keyboard.legend(action), cx).into_any_element(),
        None => div()
            .text_sm()
            .text_color(theme.group_box_foreground)
            .child(values_text(step))
            .into_any_element(),
    };
    h_flex()
        .w_full()
        .min_h(px(STEP_HEIGHT))
        .pl_1()
        .pr_1p5()
        .gap_2()
        .items_center()
        .rounded(px(STEP_RADIUS))
        .border_1()
        .border_dashed()
        .border_color(theme.danger.opacity(PREVIEW_OPACITY))
        .bg(theme.accent.opacity(PREVIEW_OPACITY))
        .child(div().flex_none().w(px(GRIP)))
        .child(number_cell(number, theme.muted_foreground))
        .child(kind_face(kind, false, false, cx))
        .child(value)
}

fn number_cell(number: usize, color: gpui_kit::Hsla) -> Div {
    div()
        .flex_none()
        .w(px(NUMBER_WIDTH))
        .text_xs()
        .text_color(color)
        .child(SharedString::from(number.to_string()))
}

fn unit(text: &'static str, color: gpui_kit::Hsla) -> Div {
    div().flex_none().text_xs().text_color(color).child(text)
}

fn values_text(step: &Step) -> String {
    Kind::of(step)
        .parts()
        .iter()
        .map(|part| {
            let mut text = String::new();
            if let Some(prefix) = part.prefix() {
                text.push_str(prefix);
                text.push(' ');
            }
            text.push_str(&part.shown(step));
            if let Some(unit) = part.unit() {
                text.push(' ');
                text.push_str(unit);
            }
            text
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn summary(step: &Step, keyboard: &Keyboard) -> SharedString {
    let kind = Kind::of(step);
    let value = match step::action(step) {
        Some(action) => keyboard.legend(action).to_string(),
        None => values_text(step),
    };
    SharedString::from(format!("{} {value}", kind.label()))
}
