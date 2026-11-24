use gpui_kit::{
    App, Context, Div, Entity, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _,
    Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        slider::{Slider, SliderState},
        switch::Switch,
        tab::{Tab, TabBar},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::keys::{RapidTrigger, ScanMode, TapDuration};

use super::{
    KeyLabels, PerformancePage,
    effective::{self, RapidTriggerMode, Selected},
};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    page::Page,
    panel,
    store::Keyboard,
    theme, usage,
};

const READOUT_WIDTH: f32 = 60.0;
const ACTION_WIDTH: f32 = 20.0;
const BAR_HEIGHT: f32 = 6.0;
const MARKER_HEIGHT: f32 = 14.0;
const MARKER_WIDTH: f32 = 2.0;
const CUSTOM_TINT: f32 = 0.28;
const LABELS: [KeyLabels; 2] = [KeyLabels::Actuation, KeyLabels::RapidTrigger];
const TAP_DURATIONS: [TapDuration; 3] = [
    TapDuration::OneMillisecond,
    TapDuration::TwentyMilliseconds,
    TapDuration::FortyMilliseconds,
];

#[derive(Debug, Clone, Copy)]
struct KeyColors {
    custom: Hsla,
    actuated: Hsla,
}

impl PerformancePage {
    pub(super) fn keyboard(&self, keyboard: &Keyboard, window: &Window, cx: &Context<Self>) -> Div {
        let colors = KeyColors::new(cx);
        let page = cx.weak_entity();
        let view = KeyboardView::new("performance-keys", keyboard, |key| {
            self.face(keyboard, key, colors)
        })
        .selection(&self.selection)
        .fit_width(panel::content_width(window))
        .on_key_click(move |key, event, window, cx| {
            page.update(cx, |page, cx| {
                page.click_key(key, event.modifiers(), window, cx);
            })
            .ok();
        });
        v_flex().w_full().gap_4().child(panel::stage(view)).when(
            self.selection.is_empty(),
            |this| {
                this.child(h_flex().w_full().justify_center().child(panel::label(
                    "Select keys to give them their own settings",
                    cx,
                )))
            },
        )
    }

    pub(super) fn heading(&self, cx: &Context<Self>) -> Div {
        let selected_custom = self.overrides.as_ref().is_some_and(|overrides| {
            self.selection
                .iter()
                .any(|key| overrides.get(key).is_some_and(effective::has_own_settings))
        });
        panel::heading(
            Page::Performance.title(),
            None,
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    TabBar::new("key-labels")
                        .segmented()
                        .small()
                        .children([
                            Tab::new().label("Actuation"),
                            Tab::new().label("Rapid Trigger"),
                        ])
                        .when_some(
                            LABELS.iter().position(|labels| *labels == self.labels),
                            TabBar::selected_index,
                        )
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            if let Some(labels) = LABELS.get(*index) {
                                this.show_labels(*labels, cx);
                            }
                        })),
                )
                .when(selected_custom, |this| {
                    this.child(
                        Button::new("reset-keys")
                            .small()
                            .label("Reset keys")
                            .tooltip("Use the profile's settings for the selected keys")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reset_keys(window, cx);
                            })),
                    )
                })
                .child(
                    Button::new("select-all")
                        .small()
                        .label("Select all keys")
                        .on_click(cx.listener(|this, _, window, cx| this.select_all(window, cx))),
                )
                .child(
                    Button::new("clear-selection")
                        .small()
                        .label("Discard selection")
                        .disabled(self.selection.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.clear_selection(window, cx);
                        })),
                ),
            cx,
        )
    }

    pub(super) fn cards(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        v_flex()
            .w_full()
            .gap_4()
            .child(card_row([
                self.actuation_card(cx),
                self.rapid_trigger_card(keyboard, cx),
                self.feedback_card(keyboard, cx),
            ]))
            .child(card_row([
                self.tachyon_card(cx),
                self.tap_duration_card(cx),
                self.keyboard_keys_card(cx),
            ]))
    }

    fn actuation_card(&self, cx: &Context<Self>) -> Div {
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return div();
        };
        let value = millimetres(self.actuation.read(cx).value().end());
        let (scope, readout, reset) = if self.selection.is_empty() {
            ("Profile".into(), value, None)
        } else {
            let entries = || {
                self.selection
                    .iter()
                    .map(|key| overrides.get(key).copied().unwrap_or_default())
            };
            let own = Selected::of(entries().map(|entry| entry.actuation.is_some()));
            let values =
                Selected::of(entries().map(|entry| effective::actuation(settings, &entry)));
            let reset = (own != Selected::Same(false)).then(|| {
                Button::new("reset-actuation")
                    .ghost()
                    .xsmall()
                    .icon(Catalog::RotateCcw)
                    .tooltip("Use the profile's actuation point")
                    .on_click(cx.listener(|this, _, window, cx| this.reset_actuation(window, cx)))
            });
            let readout = if values.is_mixed() {
                "Mixed".into()
            } else {
                value
            };
            (keys_text(self.selection.len()), readout, reset)
        };
        panel::section(
            "Actuation point",
            Some("Set the point to activate a keypress.".into()),
            cx,
        )
        .child(scope_label("Changing actuation point for", scope, cx))
        .child(slider_line(&self.actuation, readout, cx).child(action_lane(reset)))
    }

    fn rapid_trigger_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let (Some(settings), Some(overrides)) = (&self.settings, &self.overrides) else {
            return div();
        };
        let description: Option<SharedString> =
            Some("Actuates and resets keys by their direction of movement.".into());
        if self.selection.is_empty() {
            let enabled = overrides
                .iter()
                .enumerate()
                .filter(|(key, entry)| {
                    keyboard.has_key(*key) && effective::rapid_trigger(settings, entry).is_some()
                })
                .count();
            return panel::section_with(
                "Rapid Trigger",
                description,
                Switch::new("rapid-trigger")
                    .checked(settings.rapid_trigger_enabled)
                    .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                        this.set_rapid_trigger_enabled(*enabled, window, cx);
                    })),
                cx,
            )
            .child(count_label(enabled, cx))
            .children(self.sensitivity_parts(settings.rapid_trigger, false, cx));
        }
        let entries = || {
            self.selection
                .iter()
                .map(|key| overrides.get(key).copied().unwrap_or_default())
        };
        let mode = Selected::of(entries().map(|entry| RapidTriggerMode::of(entry.rapid_trigger)));
        let card = panel::section("Rapid Trigger", description, cx).child(panel::field(
            "Mode",
            h_flex()
                .gap_3()
                .items_center()
                .child(mode_tabs(mode.same(), cx))
                .when(mode.is_mixed(), |this| this.child(muted_text("Mixed", cx))),
            cx,
        ));
        if mode == Selected::Same(RapidTriggerMode::Custom) {
            let parameters =
                Selected::of(entries().map(|entry| effective::parameters(settings, &entry)));
            card.children(self.sensitivity_parts(
                parameters.first().unwrap_or(settings.rapid_trigger),
                parameters.is_mixed(),
                cx,
            ))
        } else {
            card
        }
    }

    fn feedback_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        if self.travel_unsupported {
            return panel::section(
                "Visual feedback",
                Some("The keyboard does not report key travel.".into()),
                cx,
            );
        }
        let theme = cx.theme();
        let pressed = self.pressed.and_then(|key| {
            let settings = self.settings.as_ref()?;
            let entry = self
                .overrides
                .as_ref()?
                .get(key)
                .copied()
                .unwrap_or_default();
            let travel = self.travel.get(key).copied().flatten().unwrap_or(0);
            Some((key, effective::actuation(settings, &entry), travel))
        });
        let fraction = pressed.map_or(0.0, |(_, _, travel)| effective::travel_fraction(travel));
        let reached =
            pressed.is_some_and(|(_, actuation, travel)| effective::reaches(travel, actuation));
        let readout = pressed.map_or_else(
            || "–".into(),
            |(key, _, _)| millimetres(fraction * keyboard.travel_mm(key)),
        );
        let name = pressed.map_or_else(
            || "–".into(),
            |(key, _, _)| {
                keyboard
                    .legends
                    .get(key)
                    .map_or_default(|action| SharedString::from(usage::name(*action)))
            },
        );
        let bar = div()
            .relative()
            .flex_1()
            .h(px(BAR_HEIGHT))
            .rounded_full()
            .bg(theme.secondary)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(relative(fraction))
                    .rounded_full()
                    .bg(if reached {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    }),
            )
            .when_some(pressed, |this, (_, actuation, _)| {
                this.child(
                    div()
                        .absolute()
                        .top(px((BAR_HEIGHT - MARKER_HEIGHT) / 2.0))
                        .h(px(MARKER_HEIGHT))
                        .w(px(MARKER_WIDTH))
                        .left(relative(
                            f32::from(actuation.fraction()) / f32::from(u16::MAX),
                        ))
                        .bg(theme.foreground),
                )
            });
        panel::section(
            "Visual feedback",
            Some("Press a key to test the actuation point.".into()),
            cx,
        )
        .child(panel::field(
            "Pressed key",
            div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(name),
            cx,
        ))
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .child(bar)
                .child(readout_text(readout, cx)),
        )
    }

    fn tachyon_card(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        panel::section_with(
            "Tachyon mode",
            Some("Fastest key scanning; pauses RGB effects and caps brightness at 70 %.".into()),
            Switch::new("tachyon")
                .checked(settings.scan_mode == ScanMode::Parallel)
                .on_change(cx.listener(|this, tachyon: &bool, window, cx| {
                    this.set_tachyon(*tachyon, window, cx);
                })),
            cx,
        )
    }

    fn tap_duration_card(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        panel::section(
            "Tap duration",
            Some("How long Dynamic Keystroke and Mod-Tap hold a tap.".into()),
            cx,
        )
        .child(tap_tabs(settings.tap_duration, cx))
    }

    fn keyboard_keys_card(&self, cx: &Context<Self>) -> Div {
        let Some(settings) = self.settings else {
            return div();
        };
        panel::section_with(
            "Keyboard keys",
            Some("Send keyboard keys while the keyboard runs in a gamepad mode.".into()),
            Switch::new("keyboard-in-gamepad-mode")
                .checked(settings.keyboard_in_gamepad_mode)
                .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                    this.set_keyboard_in_gamepad_mode(*enabled, window, cx);
                })),
            cx,
        )
    }

    fn sensitivity_parts(
        &self,
        rapid_trigger: RapidTrigger,
        mixed: bool,
        cx: &Context<Self>,
    ) -> Vec<Div> {
        let readout = |state: &Entity<SliderState>| {
            if mixed {
                "Mixed".into()
            } else {
                millimetres(state.read(cx).value().end())
            }
        };
        let split = rapid_trigger.release.is_some();
        let mut parts = vec![
            panel::toggle(
                "Split sensitivity",
                Some("Use a separate distance for releasing.".into()),
                Switch::new("split-sensitivity")
                    .checked(split)
                    .on_change(cx.listener(|this, split: &bool, window, cx| {
                        this.set_split(*split, window, cx);
                    })),
                cx,
            ),
            panel::field(
                "Sensitivity",
                slider_line(&self.press, readout(&self.press), cx),
                cx,
            ),
        ];
        if split {
            parts.push(panel::field(
                "Release sensitivity",
                slider_line(&self.release, readout(&self.release), cx),
                cx,
            ));
        }
        parts.push(panel::toggle(
            "Continuous",
            Some("Rapid Trigger ends only when the key is fully released.".into()),
            Switch::new("continuous")
                .checked(rapid_trigger.continuous)
                .on_change(cx.listener(|this, continuous: &bool, window, cx| {
                    this.set_continuous(*continuous, window, cx);
                })),
            cx,
        ));
        parts
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
            KeyLabels::Actuation => Some(compact_mm(keyboard.distance_mm(key, actuation))),
            KeyLabels::RapidTrigger => {
                rapid_trigger.map(|own| compact_mm(keyboard.distance_mm(key, own.press)))
            }
        };
        let fill = if travel.is_some_and(|travel| effective::reaches(travel, actuation)) {
            Some(colors.actuated)
        } else {
            effective::has_own_settings(&entry).then_some(colors.custom)
        };
        KeyFace {
            legend,
            detail: detail.map(SharedString::from),
            fill,
            travel: travel.map(effective::travel_fraction),
            dimmed: self.labels == KeyLabels::RapidTrigger && rapid_trigger.is_none(),
        }
    }
}

impl KeyColors {
    fn new(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            custom: theme::keycaps().top.blend(theme.info.opacity(CUSTOM_TINT)),
            actuated: theme.primary,
        }
    }
}

fn card_row<const CARDS: usize>(cards: [Div; CARDS]) -> Div {
    h_flex()
        .w_full()
        .items_stretch()
        .gap_4()
        .children(cards.map(|card| card.flex_1().min_w_0()))
}

fn slider_line(
    state: &Entity<SliderState>,
    readout: SharedString,
    cx: &Context<PerformancePage>,
) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(div().flex_1().min_w_0().child(Slider::new(state)))
        .child(readout_text(readout, cx))
}

fn action_lane(action: Option<Button>) -> Div {
    h_flex().w(px(ACTION_WIDTH)).justify_end().children(action)
}

fn scope_label(prefix: &'static str, value: SharedString, cx: &Context<PerformancePage>) -> Div {
    v_flex().gap_1().child(panel::label(prefix, cx)).child(
        div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().primary)
            .child(value),
    )
}

fn count_label(count: usize, cx: &Context<PerformancePage>) -> Div {
    h_flex()
        .gap_1()
        .child(panel::label("Enabled on", cx))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().primary)
                .child(count.to_string()),
        )
        .child(panel::label(if count == 1 { "key" } else { "keys" }, cx))
}

fn readout_text(text: SharedString, cx: &Context<PerformancePage>) -> Div {
    div()
        .w(px(READOUT_WIDTH))
        .text_right()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn muted_text(text: &'static str, cx: &Context<PerformancePage>) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

fn mode_tabs(current: Option<RapidTriggerMode>, cx: &Context<PerformancePage>) -> TabBar {
    TabBar::new("rapid-trigger-mode")
        .segmented()
        .small()
        .children(RapidTriggerMode::ALL.map(|mode| Tab::new().label(mode.label())))
        .when_some(
            current.and_then(|current| {
                RapidTriggerMode::ALL
                    .iter()
                    .position(|mode| *mode == current)
            }),
            TabBar::selected_index,
        )
        .on_click(cx.listener(|this, index: &usize, window, cx| {
            if let Some(mode) = RapidTriggerMode::ALL.get(*index) {
                this.set_rapid_trigger_mode(*mode, window, cx);
            }
        }))
}

fn tap_tabs(current: TapDuration, cx: &Context<PerformancePage>) -> TabBar {
    TabBar::new("tap-duration")
        .segmented()
        .small()
        .children(
            TAP_DURATIONS
                .map(|duration| Tab::new().label(format!("{} ms", duration.milliseconds()))),
        )
        .when_some(
            TAP_DURATIONS
                .iter()
                .position(|duration| *duration == current),
            TabBar::selected_index,
        )
        .on_click(cx.listener(|this, index: &usize, window, cx| {
            if let Some(duration) = TAP_DURATIONS.get(*index) {
                this.set_tap_duration(*duration, window, cx);
            }
        }))
}

fn millimetres(value: f32) -> SharedString {
    format!("{value:.2} mm").into()
}

fn keys_text(count: usize) -> SharedString {
    if count == 1 {
        "1 key".into()
    } else {
        format!("{count} keys").into()
    }
}

fn compact_mm(millimetres: f32) -> String {
    let text = format!("{millimetres:.2}");
    match text.strip_suffix('0') {
        Some(trimmed) if !trimmed.ends_with('.') => trimmed.to_owned(),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_values_keep_two_decimals_only_where_they_matter() {
        assert_eq!(compact_mm(1.2), "1.2");
        assert_eq!(compact_mm(0.15), "0.15");
        assert_eq!(compact_mm(2.0), "2.0");
        assert_eq!(compact_mm(0.996), "1.0");
        assert_eq!(compact_mm(3.25), "3.25");
    }
}
