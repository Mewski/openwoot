use gpui_kit::{
    Context, Div, Entity, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _, Styled as _,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
        button::{Button, ButtonGroup, ButtonVariants as _},
        h_flex,
        slider::{Slider, SliderState},
        switch::Switch,
        tag::Tag,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::keys::{Action, RapidTrigger, ScanMode, TapDuration};

use super::{
    KeyLabels, MIN_DISTANCE_MM, PerformancePage,
    effective::{self, RapidTriggerMode, Selected},
    reference_key,
};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    panel,
    store::Keyboard,
    usage,
};

const SLIDER_WIDTH: f32 = 240.0;
const BAR_HEIGHT: f32 = 12.0;
const OWN_SETTINGS_OPACITY: f32 = 0.45;
const TRAVEL_OPACITY: f32 = 0.35;
const TRAVEL_TOLERANCE_MM: f32 = 0.01;
const TAP_DURATIONS: [TapDuration; 3] = [
    TapDuration::OneMillisecond,
    TapDuration::TwentyMilliseconds,
    TapDuration::FortyMilliseconds,
];
const MINIMUM_NOTE: &str = "The keyboard raises any distance below the switch's minimum of \
                            0.1 mm to that minimum.";

#[derive(Debug, Clone, Copy)]
struct KeyColors {
    own: Hsla,
    actuated: Hsla,
}

impl PerformancePage {
    pub(super) fn keyboard_section(
        &self,
        keyboard: &Keyboard,
        width: Pixels,
        cx: &Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let colors = KeyColors {
            own: theme.info.opacity(OWN_SETTINGS_OPACITY),
            actuated: theme.warning,
        };
        let travel = theme.primary.opacity(TRAVEL_OPACITY);
        let page = cx.weak_entity();
        let view = KeyboardView::new("performance-keys", keyboard, |key| {
            self.face(keyboard, key, colors)
        })
        .selection(&self.selection)
        .fit_width(width)
        .on_key_click(move |key, event, window, cx| {
            page.update(cx, |page, cx| {
                page.click_key(key, event.modifiers(), window, cx);
            })
            .ok();
        });
        let description: SharedString = if self.selection.is_empty() {
            "No keys selected: the settings below apply to the whole profile. Click a key to \
             select it; Shift-, ⌘- or Ctrl-click adds and removes keys."
                .into()
        } else {
            format!(
                "{} selected: the settings below apply to them only.",
                keys_text(self.selection.len())
            )
            .into()
        };
        panel::section("Keyboard", Some(description), cx)
            .child(self.selection_bar(cx))
            .child(view)
            .child(
                h_flex()
                    .gap_4()
                    .flex_wrap()
                    .child(swatch(colors.own, "Own settings", cx))
                    .child(swatch(colors.actuated, "Past the actuation point", cx))
                    .child(swatch(travel, "Key travel", cx))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match self.labels {
                                KeyLabels::Actuation => {
                                    "Keys show their actuation point in mm; RT: Rapid Trigger on"
                                }
                                KeyLabels::RapidTrigger => {
                                    "Keys show their Rapid Trigger sensitivity in mm, \
                                     press/release when split"
                                }
                            }),
                    ),
            )
    }

    pub(super) fn actuation_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return div();
        };
        let value = self.actuation.read(cx).value().end();
        if self.selection.is_empty() {
            let travel = keyboard.travel_mm(reference_key(keyboard));
            let own = overrides
                .iter()
                .filter(|entry| entry.actuation.is_some())
                .count();
            return panel::section(
                "Actuation point",
                Some(
                    "The distance a key must be pressed before it registers a keypress, for \
                     every key without its own actuation point."
                        .into(),
                ),
                cx,
            )
            .child(panel::row(
                format!("Actuation point is {value:.2} mm"),
                Some(
                    format!(
                        "{MIN_DISTANCE_MM:.1} mm reacts to the lightest touch, {travel:.2} mm \
                         needs the key fully pressed."
                    )
                    .into(),
                ),
                slider_control(&self.actuation, None),
                cx,
            ))
            .when(travels_differ(keyboard, travel), |this| {
                this.child(panel::note(
                    "Keys whose switch travels another distance actuate at the same share of \
                     their travel.",
                    cx,
                ))
            })
            .when(own > 0, |this| {
                this.child(panel::note(
                    format!(
                        "{} their own actuation point; select them to change it.",
                        keys_have_text(own)
                    ),
                    cx,
                ))
            });
        }
        let entries = || {
            self.selection
                .iter()
                .map(|key| overrides.get(key).copied().unwrap_or_default())
        };
        let own = Selected::of(entries().map(|entry| entry.actuation.is_some()));
        let values = Selected::of(entries().map(|entry| effective::actuation(settings, &entry)));
        let profile = keyboard.distance_mm(self.selection.first().unwrap_or(0), settings.actuation);
        let status = match own {
            Selected::Same(true) => "The selected keys have their own actuation point.",
            Selected::Mixed(_) => "Some of the selected keys have their own actuation point.",
            Selected::Same(false) | Selected::Nothing => {
                "The selected keys use the actuation point of the profile."
            }
        };
        let label = if values.is_mixed() {
            "Actuation point".to_owned()
        } else {
            format!("Actuation point is {value:.2} mm")
        };
        panel::section(
            "Actuation point",
            Some(
                format!(
                    "Changing the actuation point for {}.",
                    keys_text(self.selection.len())
                )
                .into(),
            ),
            cx,
        )
        .child(panel::row(
            label,
            Some(status.into()),
            slider_control(
                &self.actuation,
                values.is_mixed().then_some("actuation-mixed"),
            ),
            cx,
        ))
        .child(panel::row(
            "Use the profile's actuation point",
            Some(format!("The profile actuates at {profile:.2} mm.").into()),
            Button::new("reset-actuation")
                .small()
                .outline()
                .label("Reset to profile")
                .disabled(own == Selected::Same(false))
                .on_click(cx.listener(|this, _, window, cx| this.reset_actuation(window, cx))),
            cx,
        ))
    }

    pub(super) fn rapid_trigger_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return div();
        };
        if self.selection.is_empty() {
            let enabled = overrides
                .iter()
                .enumerate()
                .filter(|(key, entry)| {
                    keyboard.has_key(*key) && effective::rapid_trigger(settings, entry).is_some()
                })
                .count();
            return panel::section(
                "Rapid Trigger",
                Some(
                    "Rapid Trigger dynamically actuates and resets your key based on your \
                     intention to press or release the key. Rapid Trigger starts and ends after \
                     the actuation point."
                        .into(),
                ),
                cx,
            )
            .child(panel::row(
                "Enable Rapid Trigger",
                Some(
                    format!(
                        "For every key without its own Rapid Trigger setting. Rapid Trigger is \
                         on for {}.",
                        keys_text(enabled)
                    )
                    .into(),
                ),
                Switch::new("rapid-trigger")
                    .checked(settings.rapid_trigger_enabled)
                    .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                        this.set_rapid_trigger_enabled(*enabled, window, cx);
                    })),
                cx,
            ))
            .child(self.sensitivity_rows(settings.rapid_trigger, false, cx))
            .child(panel::note(MINIMUM_NOTE, cx));
        }
        let entries = || {
            self.selection
                .iter()
                .map(|key| overrides.get(key).copied().unwrap_or_default())
        };
        let mode = Selected::of(entries().map(|entry| RapidTriggerMode::of(entry.rapid_trigger)));
        let description = match mode {
            Selected::Same(mode) => mode.description(),
            Selected::Mixed(_) | Selected::Nothing => {
                "The selected keys have different Rapid Trigger settings."
            }
        };
        let section = panel::section(
            "Rapid Trigger",
            Some(format!("Rapid Trigger for {}.", keys_text(self.selection.len())).into()),
            cx,
        )
        .child(panel::row(
            "Rapid Trigger",
            Some(description.into()),
            h_flex()
                .gap_2()
                .items_center()
                .when(mode.is_mixed(), |this| {
                    this.child(mixed_tag("rapid-trigger-mixed"))
                })
                .child(mode_buttons(mode.same(), cx)),
            cx,
        ));
        if mode == Selected::Same(RapidTriggerMode::Custom) {
            let parameters =
                Selected::of(entries().map(|entry| effective::parameters(settings, &entry)));
            section
                .child(self.sensitivity_rows(
                    parameters.first().unwrap_or(settings.rapid_trigger),
                    parameters.is_mixed(),
                    cx,
                ))
                .child(panel::note(MINIMUM_NOTE, cx))
        } else {
            section
        }
    }

    pub(super) fn feedback_section(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let section = panel::section(
            "Visual feedback",
            Some(
                "Press a key to test the actuation point. The key lights up once it passes its \
                 actuation point."
                    .into(),
            ),
            cx,
        );
        let Some(key) = self.pressed else {
            let waiting = if self.travel_unsupported {
                "The keyboard does not report the travel of its keys."
            } else if self.travel.is_empty() {
                "Waiting for the key travel of the keyboard…"
            } else {
                "Press a key to see how far it travels."
            };
            return section.child(panel::note(waiting, cx));
        };
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return section;
        };
        let theme = cx.theme();
        let entry = overrides.get(key).copied().unwrap_or_default();
        let actuation = effective::actuation(settings, &entry);
        let travel = self.travel.get(key).copied().flatten().unwrap_or(0);
        let fraction = effective::travel_fraction(travel);
        let total = keyboard.travel_mm(key);
        let name = match keyboard.legends.get(key) {
            Some(action) if *action != Action::None => usage::name(*action).into_owned(),
            _ => {
                let columns = keyboard.columns().max(1);
                format!("row {}, column {}", key / columns + 1, key % columns + 1)
            }
        };
        let rapid_trigger = effective::rapid_trigger(settings, &entry).map_or_else(
            || "Rapid Trigger off".to_owned(),
            |own| format!("Rapid Trigger {} mm", sensitivity_text(keyboard, key, own)),
        );
        section.child(
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .text_sm()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("Pressed key: {name}")),
                        )
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child(format!("{:.2} of {total:.2} mm", fraction * total)),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(BAR_HEIGHT))
                        .rounded(px(BAR_HEIGHT / 2.0))
                        .overflow_hidden()
                        .bg(theme.secondary)
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .h_full()
                                .w(relative(fraction))
                                .bg(if effective::reaches(travel, actuation) {
                                    theme.warning
                                } else {
                                    theme.primary
                                }),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .h_full()
                                .w(px(2.0))
                                .left(relative(
                                    f32::from(actuation.fraction()) / f32::from(u16::MAX),
                                ))
                                .bg(theme.foreground),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "Actuation point {:.2} mm · {rapid_trigger}",
                            keyboard.distance_mm(key, actuation)
                        )),
                ),
        )
    }

    pub(super) fn profile_section(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        panel::section(
            "Profile settings",
            Some("These settings always apply to every key of the profile.".into()),
            cx,
        )
        .child(panel::row(
            "Tachyon mode",
            Some(
                "Scans the keys in parallel for the fastest response. While it is on, effect \
                 layers and some light-bar entries pause and the brightness keys stop at 70 %."
                    .into(),
            ),
            Switch::new("tachyon")
                .checked(settings.scan_mode == ScanMode::Parallel)
                .on_change(cx.listener(|this, tachyon: &bool, window, cx| {
                    this.set_tachyon(*tachyon, window, cx);
                })),
            cx,
        ))
        .child(panel::row(
            "Tap duration",
            Some(
                "How long Dynamic Keystroke and Mod-Tap hold a tapped key. Games that read input \
                 once per tick can miss shorter taps: 20 ms suits 60-tick games, 40 ms 30-tick \
                 games."
                    .into(),
            ),
            tap_buttons(settings.tap_duration, cx),
            cx,
        ))
        .child(panel::row(
            "Keyboard keys in gamepad mode",
            Some(
                "Keep sending keyboard keys for typing in the XInput and gamepad modes. Off, they \
                 stay silent there so they cannot interfere with the controller; keep a profile \
                 key on this profile to get back to typing."
                    .into(),
            ),
            Switch::new("keyboard-in-gamepad-mode")
                .checked(settings.keyboard_in_gamepad_mode)
                .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                    this.set_keyboard_in_gamepad_mode(*enabled, window, cx);
                })),
            cx,
        ))
    }

    fn selection_bar(&self, cx: &Context<Self>) -> Div {
        let (has_own, selected_own) = self.overrides.as_ref().map_or((false, false), |overrides| {
            (
                overrides.iter().any(effective::has_own_settings),
                self.selection
                    .iter()
                    .any(|key| overrides.get(key).is_some_and(effective::has_own_settings)),
            )
        });
        h_flex()
            .w_full()
            .gap_2()
            .flex_wrap()
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("select-all")
                            .small()
                            .outline()
                            .label("Select all")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.select_all(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("select-own")
                            .small()
                            .outline()
                            .label("Select keys with own settings")
                            .disabled(!has_own)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.select_own(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("clear-selection")
                            .small()
                            .ghost()
                            .label("Clear selection")
                            .disabled(self.selection.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.clear_selection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("reset-keys")
                            .small()
                            .outline()
                            .label("Reset selected keys")
                            .tooltip("Drop the own settings of the selected keys")
                            .disabled(!selected_own)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_keys(window, cx);
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Show on keys"),
                    )
                    .child(
                        ButtonGroup::new("key-labels")
                            .small()
                            .outline()
                            .child(
                                Button::new("labels-actuation")
                                    .label("Actuation point")
                                    .selected(self.labels == KeyLabels::Actuation),
                            )
                            .child(
                                Button::new("labels-rapid-trigger")
                                    .label("Rapid Trigger")
                                    .selected(self.labels == KeyLabels::RapidTrigger),
                            )
                            .on_click(cx.listener(|this, clicks: &Vec<usize>, _, cx| {
                                let labels = if clicks.contains(&1) {
                                    KeyLabels::RapidTrigger
                                } else {
                                    KeyLabels::Actuation
                                };
                                this.show_labels(labels, cx);
                            })),
                    ),
            )
    }

    fn sensitivity_rows(
        &self,
        rapid_trigger: RapidTrigger,
        mixed: bool,
        cx: &Context<Self>,
    ) -> Div {
        let press = self.press.read(cx).value().end();
        let release = self.release.read(cx).value().end();
        let split = rapid_trigger.release.is_some();
        let (press_label, press_description) = match (mixed, split) {
            (true, _) => (
                "Sensitivity".to_owned(),
                "The selected keys have different values.",
            ),
            (false, true) => (
                format!("Press sensitivity is {press:.2} mm"),
                "How far a key moves down to press again after it passed the actuation point.",
            ),
            (false, false) => (
                format!("Sensitivity is {press:.2} mm"),
                "How far a key moves down to press again, and up to release, after it passed \
                 the actuation point.",
            ),
        };
        let release_label = if mixed {
            "Release sensitivity".to_owned()
        } else {
            format!("Release sensitivity is {release:.2} mm")
        };
        v_flex()
            .w_full()
            .child(panel::row(
                press_label,
                Some(press_description.into()),
                slider_control(&self.press, mixed.then_some("press-mixed")),
                cx,
            ))
            .child(panel::row(
                "Split sensitivity",
                Some(
                    "Separate distances for pressing and releasing, so a key can be released \
                     quickly without the risk of pressing it again."
                        .into(),
                ),
                Switch::new("split-sensitivity")
                    .checked(split)
                    .on_change(cx.listener(|this, split: &bool, window, cx| {
                        this.set_split(*split, window, cx);
                    })),
                cx,
            ))
            .when(split, |this| {
                this.child(panel::row(
                    release_label,
                    Some("How far a key moves up to release.".into()),
                    slider_control(&self.release, mixed.then_some("release-mixed")),
                    cx,
                ))
            })
            .child(panel::row(
                "Continuous Rapid Trigger",
                Some(
                    "When enabled, Rapid Trigger ends when the entire key is released. When \
                     disabled, Rapid Trigger ends at the actuation point."
                        .into(),
                ),
                Switch::new("continuous")
                    .checked(rapid_trigger.continuous)
                    .on_change(cx.listener(|this, continuous: &bool, window, cx| {
                        this.set_continuous(*continuous, window, cx);
                    })),
                cx,
            ))
    }

    fn face(&self, keyboard: &Keyboard, key: usize, colors: KeyColors) -> KeyFace {
        let legend = keyboard
            .legends
            .get(key)
            .map_or_default(|action| SharedString::from(usage::legend(*action)));
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return KeyFace {
                legend,
                ..KeyFace::default()
            };
        };
        let entry = overrides.get(key).copied().unwrap_or_default();
        let actuation = effective::actuation(settings, &entry);
        let rapid_trigger = effective::rapid_trigger(settings, &entry);
        let travel = self.travel.get(key).copied().flatten();
        let detail = match self.labels {
            KeyLabels::Actuation => {
                let millimetres = keyboard.distance_mm(key, actuation);
                if rapid_trigger.is_some() {
                    format!("{millimetres:.2} RT")
                } else {
                    format!("{millimetres:.2}")
                }
            }
            KeyLabels::RapidTrigger => rapid_trigger.map_or_else(
                || "off".to_owned(),
                |own| sensitivity_text(keyboard, key, own),
            ),
        };
        let fill = if travel.is_some_and(|travel| effective::reaches(travel, actuation)) {
            Some(colors.actuated)
        } else {
            effective::has_own_settings(&entry).then_some(colors.own)
        };
        KeyFace {
            legend,
            detail: Some(detail.into()),
            fill,
            travel: travel.map(effective::travel_fraction),
            dimmed: self.labels == KeyLabels::RapidTrigger && rapid_trigger.is_none(),
        }
    }
}

fn slider_control(state: &Entity<SliderState>, mixed: Option<&'static str>) -> Div {
    h_flex()
        .gap_2()
        .items_center()
        .when_some(mixed, |this, id| this.child(mixed_tag(id)))
        .child(div().w(px(SLIDER_WIDTH)).child(Slider::new(state)))
}

fn mixed_tag(id: &'static str) -> impl IntoElement {
    div()
        .id(id)
        .child(Tag::secondary().small().child("mix"))
        .tooltip(|window, cx| {
            Tooltip::new("The selected keys have different values.").build(window, cx)
        })
}

fn mode_buttons(current: Option<RapidTriggerMode>, cx: &Context<PerformancePage>) -> ButtonGroup {
    ButtonGroup::new("rapid-trigger-mode")
        .small()
        .outline()
        .children(RapidTriggerMode::ALL.map(|mode| {
            Button::new(mode.label())
                .label(mode.label())
                .selected(current == Some(mode))
        }))
        .on_click(cx.listener(|this, clicks: &Vec<usize>, window, cx| {
            if let Some(mode) = clicks
                .first()
                .and_then(|index| RapidTriggerMode::ALL.get(*index))
            {
                this.set_rapid_trigger_mode(*mode, window, cx);
            }
        }))
}

fn tap_buttons(current: TapDuration, cx: &Context<PerformancePage>) -> ButtonGroup {
    ButtonGroup::new("tap-duration")
        .small()
        .outline()
        .children(TAP_DURATIONS.map(|duration| {
            Button::new(SharedString::from(format!(
                "tap-{}",
                duration.milliseconds()
            )))
            .label(format!("{} ms", duration.milliseconds()))
            .selected(duration == current)
        }))
        .on_click(cx.listener(|this, clicks: &Vec<usize>, window, cx| {
            if let Some(duration) = clicks.first().and_then(|index| TAP_DURATIONS.get(*index)) {
                this.set_tap_duration(*duration, window, cx);
            }
        }))
}

fn swatch(color: Hsla, label: &'static str, cx: &Context<PerformancePage>) -> Div {
    h_flex()
        .gap_1p5()
        .items_center()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(div().size(px(10.0)).rounded(px(3.0)).bg(color))
        .child(label)
}

fn sensitivity_text(keyboard: &Keyboard, key: usize, rapid_trigger: RapidTrigger) -> String {
    let press = keyboard.distance_mm(key, rapid_trigger.press);
    match rapid_trigger.release {
        Some(release) => format!("{press:.2}/{:.2}", keyboard.distance_mm(key, release)),
        None => format!("{press:.2}"),
    }
}

fn travels_differ(keyboard: &Keyboard, travel: f32) -> bool {
    (0..keyboard.key_count())
        .filter(|key| keyboard.has_key(*key))
        .any(|key| (keyboard.travel_mm(key) - travel).abs() > TRAVEL_TOLERANCE_MM)
}

fn keys_text(count: usize) -> String {
    if count == 1 {
        "1 key".to_owned()
    } else {
        format!("{count} keys")
    }
}

fn keys_have_text(count: usize) -> String {
    if count == 1 {
        "1 key has".to_owned()
    } else {
        format!("{count} keys have")
    }
}
