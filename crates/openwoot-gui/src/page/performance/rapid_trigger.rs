use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

use super::{
    cards::{card_row, tuning_card},
    distance::{Committed, DistanceField, Shown, distance_block},
    ranges::{RELEASE_DEFAULT_MM, Span, press_default},
    selecting, tint,
    tuning::{Current, Side, Toggle, Tuning, millimetres_text, report},
};
use crate::{
    keyboard::KeyFace,
    page::Page,
    panel,
    store::{Keyboard, Store},
    switch::Switch,
    usage,
};

const PROMPT: &str = "To adjust rapid trigger, please select one or more keys first";
pub(crate) const RAPID_TRIGGER_HELP: &str =
    "Rapid Trigger dynamically adjusts the actuation and deactivation point.";
const SPLIT_HELP: &str = "Set separate activation and deactivation distances.";
const SENSITIVITY_GAP: f32 = 20.0;

pub(crate) struct RapidTriggerPage {
    store: Entity<Store>,
    tuning: Entity<Tuning>,
    press: Entity<DistanceField>,
    release: Entity<DistanceField>,
    _subscriptions: Vec<Subscription>,
}

impl RapidTriggerPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tuning = Tuning::shared(&store, cx);
        let press = cx.new(|cx| DistanceField::horizontal(window, cx));
        let release = cx.new(|cx| DistanceField::horizontal(window, cx));
        let subscriptions = vec![
            cx.observe_in(&tuning, window, |this, _, window, cx| this.sync(window, cx)),
            cx.observe_in(&store, window, |this, _, window, cx| this.sync(window, cx)),
            sensitivity_subscription(&press, Side::Press, &tuning, window, cx),
            sensitivity_subscription(&release, Side::Release, &tuning, window, cx),
        ];
        let mut page = Self {
            store,
            tuning,
            press,
            release,
            _subscriptions: subscriptions,
        };
        page.sync(window, cx);
        page
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            self.tuning.read(cx).current(keyboard)
        };
        let selected = !self.tuning.read(cx).selection().is_empty();
        if let Some(current) = current {
            show_sensitivity(&self.press, &self.release, &current, selected, window, cx);
        }
        cx.notify();
    }

    fn face(&self, keyboard: &Keyboard, key: usize, cx: &App) -> KeyFace {
        let legend = keyboard
            .legends
            .get(key)
            .map_or_default(|action| SharedString::from(usage::legend(*action)));
        let Some(own) = self
            .tuning
            .read(cx)
            .key(key)
            .and_then(|tuning| tuning.rapid_trigger)
        else {
            return KeyFace {
                legend,
                ..KeyFace::default()
            };
        };
        let span = Span::sensitivity(keyboard.travel_mm(key));
        let press = keyboard.distance_mm(key, own.press);
        let release = own
            .release
            .map(|release| keyboard.distance_mm(key, release));
        KeyFace {
            legend,
            detail: Some(millimetres_text(press)),
            corner: release.map(millimetres_text),
            fill: Some(tint::of(span.fraction(press))),
            lower_fill: release.map(|release| tint::of(span.fraction(release))),
            ..KeyFace::default()
        }
    }

    fn enable_card(&self, keyboard: &Keyboard, current: &Current, cx: &Context<Self>) -> Div {
        let selected = !self.tuning.read(cx).selection().is_empty();
        let enabled = self.tuning.read(cx).rapid_trigger_keys(keyboard);
        let tuning = self.tuning.downgrade();
        let switch = Switch::new("rapid-trigger")
            .checked(current.rapid_trigger.is_on())
            .mixed(current.rapid_trigger.is_mixed())
            .disabled(!selected)
            .on_change(move |enabled: &bool, window, cx| {
                if let Ok(changed) =
                    tuning.update(cx, |tuning, cx| tuning.set_rapid_trigger(*enabled, cx))
                {
                    report(changed, window, cx);
                }
            });
        tuning_card(
            panel::section_with(
                "Enable Rapid Trigger",
                Some(
                    "Actuates and resets your key based on your intention to press or release it."
                        .into(),
                ),
                switch,
                cx,
            )
            .child(enabled_count(enabled, cx)),
            selected,
        )
    }

    fn sensitivity_card(&self, current: &Current, cx: &Context<Self>) -> Div {
        let enabled = current.rapid_trigger.is_on() && !self.tuning.read(cx).selection().is_empty();
        tuning_card(
            panel::section("Rapid Trigger Sensitivity", None, cx).child(sensitivity_group(
                &self.tuning,
                &self.press,
                &self.release,
                current,
                enabled,
                cx,
            )),
            enabled,
        )
    }

    fn continuous_card(&self, current: &Current, cx: &Context<Self>) -> Div {
        let enabled = current.rapid_trigger.is_on() && !self.tuning.read(cx).selection().is_empty();
        let tuning = self.tuning.downgrade();
        let switch = Switch::new("continuous")
            .checked(current.continuous.is_on())
            .mixed(current.continuous.is_mixed())
            .disabled(!enabled)
            .on_change(move |continuous: &bool, window, cx| {
                if let Ok(changed) =
                    tuning.update(cx, |tuning, cx| tuning.set_continuous(*continuous, cx))
                {
                    report(changed, window, cx);
                }
            });
        tuning_card(
            panel::section_with(
                "Continuous Rapid Trigger",
                Some("When enabled, Rapid Trigger ends when the entire key is released.".into()),
                switch,
                cx,
            )
            .child(div().flex_1())
            .child(panel::info(
                "Spam a key continuously without worrying about the actuation point.",
                cx,
            )),
            enabled,
        )
    }
}

impl Render for RapidTriggerPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return page;
        };
        let Some(current) = self.tuning.read(cx).current(keyboard) else {
            return page.when(self.tuning.read(cx).profile().is_some(), |page| {
                page.child(panel::note("Key settings unavailable", cx))
            });
        };
        let selected = self.tuning.read(cx).selection().len();
        let view = selecting::tuning_keyboard(
            "rapid-trigger-keys",
            keyboard,
            &self.tuning,
            |key| self.face(keyboard, key, cx),
            cx,
        );
        panel::page_with_stage(
            "rapid-trigger-page",
            selecting::stage(view, selected, PROMPT),
            panel::heading(Page::RapidTrigger.title())
                .help(RAPID_TRIGGER_HELP)
                .trailing(selecting::tuning_actions(&self.tuning, keyboard, cx)),
            card_row()
                .child(self.enable_card(keyboard, &current, cx).flex_1().min_w_0())
                .child(self.sensitivity_card(&current, cx).flex_1().min_w_0())
                .child(self.continuous_card(&current, cx).flex_1().min_w_0()),
        )
    }
}

pub(crate) fn sensitivity_subscription<T: 'static>(
    field: &Entity<DistanceField>,
    side: Side,
    tuning: &Entity<Tuning>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Subscription {
    let tuning = tuning.downgrade();
    cx.subscribe_in(
        field,
        window,
        move |_, _, Committed(millimetres): &Committed, window, cx| {
            if let Ok(changed) = tuning.update(cx, |tuning, cx| {
                tuning.set_sensitivity(side, *millimetres, cx)
            }) {
                report(changed, window, cx);
            }
        },
    )
}

pub(crate) fn show_sensitivity(
    press: &Entity<DistanceField>,
    release: &Entity<DistanceField>,
    current: &Current,
    selected: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let disabled = !selected || current.rapid_trigger != Toggle::On;
    let split = current.split.is_on();
    press.update(cx, |field, cx| {
        field.show(
            Shown {
                millimetres: current.press.millimetres,
                span: current.press.span,
                default: Some(press_default(split)),
                mixed: current.press.mixed,
                disabled,
            },
            window,
            cx,
        );
    });
    release.update(cx, |field, cx| {
        field.show(
            Shown {
                millimetres: current.release.millimetres,
                span: current.release.span,
                default: Some(RELEASE_DEFAULT_MM),
                mixed: current.release.mixed,
                disabled,
            },
            window,
            cx,
        );
    });
}

pub(crate) fn sensitivity_group(
    tuning: &Entity<Tuning>,
    press: &Entity<DistanceField>,
    release: &Entity<DistanceField>,
    current: &Current,
    enabled: bool,
    cx: &App,
) -> Div {
    let split = current.split;
    let toggling = tuning.downgrade();
    v_flex()
        .w_full()
        .gap(px(SENSITIVITY_GAP))
        .child(panel::toggle(
            "Split sensitivity",
            Some(SPLIT_HELP.into()),
            Switch::new("split-sensitivity")
                .checked(split.is_on())
                .mixed(split.is_mixed())
                .disabled(!enabled)
                .on_change(move |split: &bool, window, cx| {
                    if let Ok(changed) =
                        toggling.update(cx, |tuning, cx| tuning.set_split(*split, cx))
                    {
                        report(changed, window, cx);
                    }
                }),
            cx,
        ))
        .child(distance_block(
            if split.is_on() {
                "Key press - activate"
            } else {
                "Sensitivity"
            },
            press,
            cx,
        ))
        .when(split.is_on(), |this| {
            this.child(distance_block("Key release - deactivate", release, cx))
        })
}

fn enabled_count(count: usize, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .gap_1()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(theme.group_box_foreground)
        .child("ENABLED ON")
        .child(div().text_color(theme.primary).child(count.to_string()))
        .child(if count == 1 { "KEY" } else { "KEYS" })
}
