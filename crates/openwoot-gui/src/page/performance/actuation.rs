use gpui_kit::{
    App, AppContext as _, Axis, Context, Div, Entity, FontWeight, Hsla, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, relative,
};

use super::{
    cards::{art_slot, card_row, tuning_card},
    distance::{Committed, DistanceField, Shown},
    effective, feedback,
    ranges::Span,
    selecting, switch_art, tint,
    travel::TravelFeed,
    tuning::{Tuning, millimetres_text, report},
};
use crate::{
    keyboard::KeyFace,
    page::Page,
    panel,
    store::{Keyboard, Store},
    usage,
};

const PROMPT: &str = "To adjust actuation point, please select one or more keys first";
pub(crate) const ACTUATION_HELP: &str =
    "Actuation point is the distance a key must be pressed to register a keystroke.";
const COLUMN_WIDTH: f32 = 760.0;
const CARD_MAX_WIDTH: f32 = 380.0;
const VALUE_LINE_HEIGHT: f32 = 1.4;

pub(crate) struct ActuationPage {
    store: Entity<Store>,
    tuning: Entity<Tuning>,
    feed: Entity<TravelFeed>,
    field: Entity<DistanceField>,
    _subscriptions: Vec<Subscription>,
}

impl ActuationPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tuning = Tuning::shared(&store, cx);
        let feed = TravelFeed::shared(&store, cx);
        let field = cx.new(|cx| DistanceField::vertical(window, cx));
        let subscriptions = vec![
            cx.observe_in(&tuning, window, |this, _, window, cx| this.sync(window, cx)),
            cx.observe_in(&store, window, |this, _, window, cx| this.sync(window, cx)),
            cx.observe(&feed, |_, _, cx| cx.notify()),
            cx.subscribe_in(
                &field,
                window,
                |this, _, Committed(millimetres): &Committed, window, cx| {
                    let changed = this
                        .tuning
                        .update(cx, |tuning, cx| tuning.set_actuation(*millimetres, cx));
                    report(changed, window, cx);
                },
            ),
        ];
        let mut page = Self {
            store,
            tuning,
            feed,
            field,
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
        let disabled = self.tuning.read(cx).selection().is_empty();
        if let Some(current) = current {
            self.field.update(cx, |field, cx| {
                field.show(
                    Shown {
                        millimetres: current.actuation.millimetres,
                        span: current.actuation.span,
                        default: None,
                        mixed: current.actuation.mixed,
                        disabled,
                    },
                    window,
                    cx,
                );
            });
        }
        cx.notify();
    }

    fn face(&self, keyboard: &Keyboard, key: usize, cx: &App) -> KeyFace {
        let legend = keyboard
            .legends
            .get(key)
            .map_or_default(|action| SharedString::from(usage::legend(*action)));
        let Some(tuning) = self.tuning.read(cx).key(key) else {
            return KeyFace {
                legend,
                ..KeyFace::default()
            };
        };
        let millimetres = keyboard.distance_mm(key, tuning.actuation);
        let travel = self.feed.read(cx).travel(key);
        let actuated = travel.is_some_and(|travel| effective::reaches(travel, tuning.actuation));
        let span = Span::actuation(keyboard.travel_mm(key));
        KeyFace {
            legend,
            detail: Some(millimetres_text(millimetres)),
            fill: Some(if actuated {
                cx.theme().primary
            } else {
                tint::of(span.fraction(millimetres))
            }),
            travel: travel.map(effective::travel_fraction),
            ..KeyFace::default()
        }
    }

    fn setter(&self, keyboard: &Keyboard, cx: &App) -> Div {
        let selected = self.tuning.read(cx).selection().len();
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
        tuning_card(
            panel::section(
                "Set Actuation Point",
                Some(
                    "Set the exact distance a key must be pressed before it registers a keypress."
                        .into(),
                ),
                cx,
            )
            .child(
                v_flex()
                    .child(strong_label("Changing actuation point for", cx))
                    .child(value(keys_text(selected), cx.theme().primary)),
            )
            .child(
                h_flex()
                    .gap_5()
                    .items_center()
                    .child(art_slot(switch_art::switch(depth, cx.theme().success, cx)))
                    .child(self.field.clone()),
            ),
            selected > 0,
        )
    }

    fn feedback(&self, keyboard: &Keyboard, cx: &App) -> Div {
        let feed = self.feed.read(cx);
        if feed.is_unsupported() {
            return tuning_card(
                panel::section(
                    "Visual Feedback",
                    Some("This keyboard does not report key travel.".into()),
                    cx,
                ),
                false,
            );
        }
        let theme = cx.theme();
        let pressed = feedback::pressed_key(feed, self.tuning.read(cx), keyboard);
        let pressed = pressed.as_ref();
        tuning_card(
            panel::section(
                "Visual Feedback",
                Some("Press a key to test the actuation point.".into()),
                cx,
            )
            .child(v_flex().child(strong_label("Pressed key", cx)).child(value(
                pressed.map_or_else(|| "–".into(), |pressed| pressed.name.clone()),
                theme.foreground,
            )))
            .child(
                h_flex()
                    .gap_5()
                    .items_center()
                    .child(art_slot(switch_art::switch(
                        pressed.map_or(0.0, |pressed| pressed.travel),
                        feedback::meter_color(pressed, cx),
                        cx,
                    )))
                    .child(feedback::meter(Axis::Vertical, pressed, cx))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.group_box_foreground)
                            .child(
                                pressed
                                    .map_or_else(|| "–".into(), |pressed| pressed.readout.clone()),
                            ),
                    ),
            ),
            true,
        )
    }
}

impl Render for ActuationPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let view = selecting::tuning_keyboard(
            "actuation-keys",
            keyboard,
            &self.tuning,
            |key| self.face(keyboard, key, cx),
            cx,
        );
        panel::page_with_stage(
            "actuation-page",
            selecting::stage(view, selected, PROMPT),
            h_flex().w_full().justify_center().child(
                div().w_full().max_w(px(COLUMN_WIDTH)).child(
                    panel::heading(Page::Actuation.title())
                        .help(ACTUATION_HELP)
                        .trailing(selecting::tuning_actions(&self.tuning, keyboard, cx)),
                ),
            ),
            h_flex().w_full().justify_center().child(
                card_row()
                    .max_w(px(COLUMN_WIDTH))
                    .child(
                        self.setter(keyboard, cx)
                            .flex_1()
                            .min_w_0()
                            .max_w(px(CARD_MAX_WIDTH)),
                    )
                    .child(
                        self.feedback(keyboard, cx)
                            .flex_1()
                            .min_w_0()
                            .max_w(px(CARD_MAX_WIDTH)),
                    ),
            ),
        )
    }
}

fn keys_text(count: usize) -> SharedString {
    if count == 1 {
        "1 key".into()
    } else {
        format!("{count} keys").into()
    }
}

fn strong_label(text: &'static str, cx: &App) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().foreground)
        .child(SharedString::from(text.to_uppercase()))
}

fn value(text: SharedString, color: Hsla) -> Div {
    div()
        .text_base()
        .line_height(relative(VALUE_LINE_HEIGHT))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_counts_read_naturally() {
        assert_eq!(keys_text(0), "0 keys");
        assert_eq!(keys_text(1), "1 key");
        assert_eq!(keys_text(84), "84 keys");
    }
}
