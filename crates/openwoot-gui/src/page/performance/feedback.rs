use gpui_kit::{
    App, Axis, Div, Hsla, ParentElement as _, SharedString, Styled as _,
    component::ActiveTheme as _, div, prelude::FluentBuilder as _, px, relative,
};

use super::{
    effective,
    travel::TravelFeed,
    tuning::{Tuning, millimetres_text},
};
use crate::store::Keyboard;

const METER_LENGTH: f32 = 100.0;
const METER_WIDTH: f32 = 4.0;
const MARK_LENGTH: f32 = 16.0;
const MARK_WIDTH: f32 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pressed {
    pub(crate) name: SharedString,
    pub(crate) travel: f32,
    pub(crate) actuation: f32,
    pub(crate) reached: bool,
    pub(crate) readout: SharedString,
}

pub(crate) fn pressed_key(
    feed: &TravelFeed,
    tuning: &Tuning,
    keyboard: &Keyboard,
) -> Option<Pressed> {
    let (key, travel) = feed.pressed()?;
    let actuation = tuning.key(key)?.actuation;
    let fraction = effective::travel_fraction(travel);
    Some(Pressed {
        name: keyboard
            .legends
            .get(key)
            .map_or_default(|action| keyboard.legend(*action)),
        travel: fraction,
        actuation: f32::from(actuation.fraction()) / f32::from(u16::MAX),
        reached: effective::reaches(travel, actuation),
        readout: format!(
            "{} mm",
            millimetres_text(fraction * keyboard.travel_mm(key))
        )
        .into(),
    })
}

pub(crate) fn meter_color(pressed: Option<&Pressed>, cx: &App) -> Hsla {
    if pressed.is_some_and(|pressed| pressed.reached) {
        cx.theme().primary
    } else {
        cx.theme().muted_foreground
    }
}

pub(crate) fn meter(axis: Axis, pressed: Option<&Pressed>, cx: &App) -> Div {
    let theme = cx.theme();
    let fill = meter_color(pressed, cx);
    let travel = pressed.map_or(0.0, |pressed| pressed.travel);
    div()
        .relative()
        .rounded_full()
        .bg(theme.muted_foreground.opacity(0.3))
        .map(|this| match axis {
            Axis::Vertical => this.flex_none().w(px(METER_WIDTH)).h(px(METER_LENGTH)),
            Axis::Horizontal => this.flex_1().min_w_0().h(px(METER_WIDTH)),
        })
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .rounded_full()
                .bg(fill)
                .map(|this| match axis {
                    Axis::Vertical => this.w_full().h(relative(travel)),
                    Axis::Horizontal => this.h_full().w(relative(travel)),
                }),
        )
        .when_some(pressed, |this, pressed| {
            let at = relative(pressed.actuation);
            let offset = px((METER_WIDTH - MARK_LENGTH) / 2.0);
            this.child(div().absolute().bg(theme.foreground).map(|this| {
                match axis {
                    Axis::Vertical => this
                        .left(offset)
                        .top(at)
                        .w(px(MARK_LENGTH))
                        .h(px(MARK_WIDTH)),
                    Axis::Horizontal => this
                        .top(offset)
                        .left(at)
                        .h(px(MARK_LENGTH))
                        .w(px(MARK_WIDTH)),
                }
            }))
        })
}
