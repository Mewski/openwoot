use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, FontWeight, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Window,
    assets::IconName,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
};

use super::{
    distance::{Committed, DistanceField, Shown},
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

const PROMPT: &str = "To adjust Rapid Trigger, select one or more keys first";
pub(crate) const RAPID_TRIGGER_HELP: &str = "Rapid Trigger moves the actuation and reset points with your finger: a key activates the moment you start pressing it and resets the moment you start releasing it.\n\nIt speeds up competitive play, since you never have to release a key fully before pressing it again.";
const SPLIT_HELP: &str = "If Rapid Trigger releases a key too eagerly in intense play, set separate distances to activate and to release, so you can let go of a key quickly without pressing it again by accident.";

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
        panel::section_with(
            "Enable Rapid Trigger",
            Some("Actuates and resets keys by the direction they move, starting after the actuation point.".into()),
            switch,
            cx,
        )
        .child(enabled_count(enabled, cx))
        .when(!selected, |this| this.opacity(0.5))
    }

    fn sensitivity_card(&self, current: &Current, cx: &Context<Self>) -> Div {
        let enabled = current.rapid_trigger.is_on() && !self.tuning.read(cx).selection().is_empty();
        panel::section("Rapid Trigger sensitivity", None, cx)
            .children(sensitivity_parts(
                &self.tuning,
                &self.press,
                &self.release,
                current,
                enabled,
                cx,
            ))
            .when(!enabled, |this| this.opacity(0.5))
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
        let theme = cx.theme();
        panel::section_with(
            "Continuous Rapid Trigger",
            Some("When on, Rapid Trigger ends only when the key is fully released; when off, it ends at the actuation point.".into()),
            switch,
            cx,
        )
        .child(div().flex_1())
        .child(
            h_flex()
                .gap_3()
                .items_start()
                .p_3()
                .rounded(theme.radius)
                .bg(theme.secondary)
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.info)
                        .child(Icon::new(IconName::Info)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Turn it on to spam a key without minding the actuation point."),
                ),
        )
        .when(!enabled, |this| this.opacity(0.5))
    }
}

impl Render for RapidTriggerPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            window,
            cx,
        );
        panel::page_with_stage(
            "rapid-trigger-page",
            selecting::stage(view, selected, PROMPT, cx),
            v_flex()
                .w_full()
                .gap_5()
                .child(panel::heading(
                    Page::RapidTrigger.title(),
                    Some(RAPID_TRIGGER_HELP.into()),
                    selecting::tuning_actions(&self.tuning, keyboard, cx),
                    cx,
                ))
                .child(
                    h_flex()
                        .w_full()
                        .items_stretch()
                        .gap_4()
                        .child(self.enable_card(keyboard, &current, cx).flex_1().min_w_0())
                        .child(self.sensitivity_card(&current, cx).flex_1().min_w_0())
                        .child(self.continuous_card(&current, cx).flex_1().min_w_0()),
                ),
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

pub(crate) fn sensitivity_parts(
    tuning: &Entity<Tuning>,
    press: &Entity<DistanceField>,
    release: &Entity<DistanceField>,
    current: &Current,
    enabled: bool,
    cx: &App,
) -> Vec<Div> {
    let split = current.split;
    let toggling = tuning.downgrade();
    let mut parts = vec![
        panel::toggle(
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
        ),
        field_label(
            if split.is_on() {
                "Key press - activate"
            } else {
                "Sensitivity"
            },
            cx,
        )
        .child(press.clone()),
    ];
    if split.is_on() {
        parts.push(field_label("Key release - deactivate", cx).child(release.clone()));
    }
    parts
}

fn field_label(text: &'static str, cx: &App) -> Div {
    v_flex().w_full().gap_1().child(
        div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().foreground)
            .child(SharedString::from(text.to_uppercase())),
    )
}

fn enabled_count(count: usize, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .gap_1()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.foreground)
        .child("ENABLED ON")
        .child(div().text_color(theme.primary).child(count.to_string()))
        .child(if count == 1 { "KEY" } else { "KEYS" })
}
