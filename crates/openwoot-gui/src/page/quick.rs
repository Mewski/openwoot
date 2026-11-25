mod scan_rate;

use std::time::{Duration, Instant};

use gpui_kit::{
    App, AppContext as _, Axis, Context, Div, Entity, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    rgb,
};
use openwoot_protocol::{
    endpoints::{GetDiagnostics, GetKeyColors},
    keys::Layer,
    lighting::{KeyColors, Rgb},
    profile::{LayerRef, Slot},
};

use self::scan_rate::ScanRate;
use super::{
    Page,
    performance::{
        ACTUATION_HELP, Committed, DistanceField, Pressed, RAPID_TRIGGER_HELP, Shown, Side,
        TravelFeed, Tuning, Visibility, meter, millimetres_text, pressed_key, report,
        sensitivity_parts, sensitivity_subscription, show_sensitivity, stage, switch,
        tuning_actions, tuning_keyboard,
    },
};
use crate::{
    keyboard::KeyFace,
    panel,
    store::{Keyboard, Store, StoreEvent},
    switch::Switch,
    usage,
};

const SCAN_INTERVAL: Duration = Duration::from_secs(1);
const PROMPT: &str = "To adjust actuation point and Rapid Trigger, select one or more keys first";
const SCAN_RATE_HELP: &str = "How many times per second the keyboard reads every key, measured live. A higher scan rate makes the keyboard more responsive.";

pub(crate) struct QuickPage {
    store: Entity<Store>,
    tuning: Entity<Tuning>,
    feed: Entity<TravelFeed>,
    actuation: Entity<DistanceField>,
    press: Entity<DistanceField>,
    release: Entity<DistanceField>,
    colors: Option<(Slot, KeyColors)>,
    hovered: Option<Card>,
    scan_rate: ScanRate,
    visibility: Visibility,
    polling: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Card {
    Actuation,
    RapidTrigger,
}

impl QuickPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tuning = Tuning::shared(&store, cx);
        let feed = TravelFeed::shared(&store, cx);
        let actuation = cx.new(|cx| DistanceField::vertical(window, cx));
        let press = cx.new(|cx| DistanceField::horizontal(window, cx));
        let release = cx.new(|cx| DistanceField::horizontal(window, cx));
        let changing = tuning.downgrade();
        let subscriptions = vec![
            cx.observe_in(&tuning, window, |this, _, window, cx| this.sync(window, cx)),
            cx.observe_in(&store, window, |this, _, window, cx| this.sync(window, cx)),
            cx.observe(&feed, |_, _, cx| cx.notify()),
            cx.subscribe_in(&store, window, |this, _, event, _, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.load_colors(cx);
                }
                StoreEvent::Disconnected | StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &actuation,
                window,
                move |_, _, Committed(millimetres): &Committed, window, cx| {
                    if let Ok(changed) =
                        changing.update(cx, |tuning, cx| tuning.set_actuation(*millimetres, cx))
                    {
                        report(changed, window, cx);
                    }
                },
            ),
            sensitivity_subscription(&press, Side::Press, &tuning, window, cx),
            sensitivity_subscription(&release, Side::Release, &tuning, window, cx),
        ];
        let mut page = Self {
            store,
            tuning,
            feed,
            actuation,
            press,
            release,
            colors: None,
            hovered: None,
            scan_rate: ScanRate::default(),
            visibility: Visibility::new(Instant::now()),
            polling: false,
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
            self.actuation.update(cx, |field, cx| {
                field.show(
                    Shown {
                        millimetres: current.actuation.millimetres,
                        span: current.actuation.span,
                        default: None,
                        mixed: current.actuation.mixed,
                        disabled: !selected,
                    },
                    window,
                    cx,
                );
            });
            show_sensitivity(&self.press, &self.release, &current, selected, window, cx);
        }
        cx.notify();
    }

    fn load_colors(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let request = self
            .store
            .read(cx)
            .device()
            .configure::<GetKeyColors, Option<KeyColors>>(LayerRef {
                profile,
                layer: Layer::Base,
            });
        cx.spawn(async move |this, cx| {
            let colors = request.await.ok().flatten();
            this.update(cx, |page, cx| {
                page.colors = colors.map(|colors| (profile, colors));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn keep_polling(&mut self, cx: &mut Context<Self>) {
        if self.polling {
            self.visibility.rendered(Instant::now());
            return;
        }
        self.polling = true;
        self.visibility.restart(Instant::now());
        self.load_colors(cx);
        let device = self.store.read(cx).device();
        cx.spawn(async move |this, cx| {
            loop {
                let started = Instant::now();
                let shown = this
                    .update(cx, |page, _| page.visibility.is_shown(started))
                    .unwrap_or(false);
                if !shown {
                    this.update(cx, |page, _| page.stop_polling()).ok();
                    return;
                }
                let report = device.call::<GetDiagnostics>(()).await;
                if this
                    .update(cx, |page, cx| {
                        if let Ok(report) = report {
                            page.scan_rate.sample(report.scan.cycles, Instant::now());
                        }
                        page.visibility.notified(Instant::now());
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
                cx.background_executor()
                    .timer(SCAN_INTERVAL.saturating_sub(started.elapsed()))
                    .await;
            }
        })
        .detach();
    }

    fn stop_polling(&mut self) {
        self.polling = false;
        self.scan_rate.forget();
    }

    fn hover(&mut self, card: Card, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.hovered = Some(card);
        } else if self.hovered == Some(card) {
            self.hovered = None;
        }
        cx.notify();
    }

    fn face(&self, keyboard: &Keyboard, key: usize, cx: &App) -> KeyFace {
        let legend = keyboard
            .legends
            .get(key)
            .map_or_default(|action| SharedString::from(usage::legend(*action)));
        let fill = self
            .colors
            .as_ref()
            .filter(|(profile, _)| Some(*profile) == self.store.read(cx).active())
            .and_then(|(_, colors)| colors.get(key).copied())
            .map(hsla_of);
        let tuning = self.tuning.read(cx).key(key);
        let face = KeyFace {
            legend,
            fill,
            ..KeyFace::default()
        };
        match (self.hovered, tuning) {
            (Some(Card::Actuation), Some(tuning)) => KeyFace {
                legend: millimetres_text(keyboard.distance_mm(key, tuning.actuation)),
                ..face
            },
            (Some(Card::RapidTrigger), Some(tuning)) => match tuning.rapid_trigger {
                Some(own) => {
                    let press = millimetres_text(keyboard.distance_mm(key, own.press));
                    let legend = match own.release {
                        Some(release) => format!(
                            "{press}\n{}",
                            millimetres_text(keyboard.distance_mm(key, release))
                        )
                        .into(),
                        None => press,
                    };
                    KeyFace { legend, ..face }
                }
                None => KeyFace {
                    dimmed: true,
                    ..face
                },
            },
            _ => face,
        }
    }

    fn actuation_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let selected = !self.tuning.read(cx).selection().is_empty();
        let depth = self
            .tuning
            .read(cx)
            .current(keyboard)
            .map_or(0.0, |current| {
                current
                    .actuation
                    .span
                    .fraction(current.actuation.millimetres)
            });
        let feed = self.feed.read(cx);
        let pressed =
            (!feed.is_unsupported()).then(|| pressed_key(feed, self.tuning.read(cx), keyboard));
        panel::section_with_help(
            "Actuation point",
            ACTUATION_HELP,
            Some("Set the point to activate a keypress for all selected keys.".into()),
            None,
            cx,
        )
        .child(
            h_flex()
                .gap_6()
                .items_center()
                .child(switch(depth, cx.theme().success, cx))
                .child(self.actuation.clone())
                .when(!selected, |this| this.opacity(0.5)),
        )
        .when_some(pressed, |this, pressed| {
            this.child(panel::field(
                "Visual feedback",
                feedback_row(pressed.as_ref(), cx),
                cx,
            ))
        })
    }

    fn rapid_trigger_card(&self, keyboard: &Keyboard, cx: &Context<Self>) -> Div {
        let selected = !self.tuning.read(cx).selection().is_empty();
        let Some(current) = self.tuning.read(cx).current(keyboard) else {
            return div();
        };
        let tuning = self.tuning.downgrade();
        let enabled = selected && current.rapid_trigger.is_on();
        panel::section_with_help(
            "Rapid Trigger",
            RAPID_TRIGGER_HELP,
            Some("When on, a key activates and resets by the direction it moves.".into()),
            Some(
                Switch::new("quick-rapid-trigger")
                    .checked(current.rapid_trigger.is_on())
                    .mixed(current.rapid_trigger.is_mixed())
                    .disabled(!selected)
                    .on_change(move |enabled: &bool, window, cx| {
                        if let Ok(changed) =
                            tuning.update(cx, |tuning, cx| tuning.set_rapid_trigger(*enabled, cx))
                        {
                            report(changed, window, cx);
                        }
                    })
                    .into_any_element(),
            ),
            cx,
        )
        .children(sensitivity_parts(
            &self.tuning,
            &self.press,
            &self.release,
            &current,
            enabled,
            cx,
        ))
        .when(!selected, |this| this.opacity(0.5))
    }

    fn tachyon_card(&self, cx: &Context<Self>) -> Div {
        let tachyon = self.tuning.read(cx).tachyon().unwrap_or(false);
        let tuning = self.tuning.downgrade();
        let theme = cx.theme();
        panel::section_with(
            "Tachyon mode",
            Some("Scans keys fastest; caps brightness at 70 % and pauses RGB effects.".into()),
            Switch::new("tachyon")
                .checked(tachyon)
                .on_change(move |tachyon: &bool, window, cx| {
                    if let Ok(changed) =
                        tuning.update(cx, |tuning, cx| tuning.set_tachyon(*tachyon, cx))
                    {
                        report(changed, window, cx);
                    }
                }),
            cx,
        )
        .when_some(self.scan_rate.hertz(), |this, hertz| {
            this.child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .id("scan-rate")
                            .gap_1()
                            .items_center()
                            .child(panel::label("Scan rate", cx))
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .child(Icon::new(IconName::CircleQuestionMark).small()),
                            )
                            .tooltip(|window, cx| Tooltip::new(SCAN_RATE_HELP).build(window, cx)),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.primary)
                            .child(format!("{hertz} Hz")),
                    ),
            )
        })
    }
}

impl Render for QuickPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.keep_polling(cx);
        self.feed.update(cx, TravelFeed::keep_alive);
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return page;
        };
        if !self.tuning.read(cx).is_loaded() {
            return page.when(self.tuning.read(cx).profile().is_some(), |page| {
                page.child(panel::note("Key settings unavailable", cx))
            });
        }
        let selected = self.tuning.read(cx).selection().len();
        let view = tuning_keyboard(
            "quick-keys",
            keyboard,
            &self.tuning,
            |key| self.face(keyboard, key, cx),
            window,
            cx,
        );
        panel::page_with_stage(
            "quick-page",
            stage(view, selected, PROMPT, cx),
            v_flex()
                .w_full()
                .gap_5()
                .child(panel::heading(
                    Page::Quick.title(),
                    None,
                    tuning_actions(&self.tuning, keyboard, cx),
                    cx,
                ))
                .child(
                    h_flex()
                        .w_full()
                        .items_stretch()
                        .gap_4()
                        .child(
                            self.actuation_card(keyboard, cx)
                                .id("quick-actuation")
                                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                    this.hover(Card::Actuation, *hovered, cx);
                                }))
                                .flex_1()
                                .min_w_0(),
                        )
                        .child(
                            self.rapid_trigger_card(keyboard, cx)
                                .id("quick-rapid-trigger")
                                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                    this.hover(Card::RapidTrigger, *hovered, cx);
                                }))
                                .flex_1()
                                .min_w_0(),
                        )
                        .child(self.tachyon_card(cx).flex_1().min_w_0()),
                ),
        )
    }
}

fn feedback_row(pressed: Option<&Pressed>, cx: &App) -> Div {
    let theme = cx.theme();
    let Some(pressed) = pressed else {
        return div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child("Press a key to test the actuation point.");
    };
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(
            div()
                .flex_none()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(pressed.name.clone()),
        )
        .child(meter(Axis::Horizontal, Some(pressed), cx))
        .child(
            div()
                .flex_none()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(pressed.readout.clone()),
        )
}

fn hsla_of(color: Rgb) -> Hsla {
    rgb(u32::from(color.red) << 16 | u32::from(color.green) << 8 | u32::from(color.blue)).into()
}
