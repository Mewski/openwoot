use std::{ops::RangeInclusive, rc::Rc};

use gpui_kit::{
    App, Div, ParentElement as _, SharedString, Styled as _, Window,
    component::{
        Disableable as _, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
    },
    div, px,
};

const LABEL_WIDTH: f32 = 84.0;

pub(crate) fn stepper(
    id: impl Into<SharedString>,
    value: u8,
    bounds: RangeInclusive<u8>,
    label: impl Into<SharedString>,
    on_change: impl Fn(&u8, &mut Window, &mut App) + 'static,
) -> Div {
    let id: SharedString = id.into();
    let on_change = Rc::new(on_change);
    let on_lower = Rc::clone(&on_change);
    let (min, max) = (*bounds.start(), *bounds.end());
    let lower = value.saturating_sub(1).clamp(min, max);
    let higher = value.saturating_add(1).clamp(min, max);
    h_flex()
        .gap_1()
        .items_center()
        .child(
            Button::new(SharedString::from(format!("{id}-lower")))
                .icon(IconName::Minus)
                .ghost()
                .xsmall()
                .disabled(value <= min)
                .on_click(move |_, window, cx| on_lower(&lower, window, cx)),
        )
        .child(
            div()
                .min_w(px(LABEL_WIDTH))
                .text_center()
                .text_sm()
                .child(label.into()),
        )
        .child(
            Button::new(SharedString::from(format!("{id}-higher")))
                .icon(IconName::Plus)
                .ghost()
                .xsmall()
                .disabled(value >= max)
                .on_click(move |_, window, cx| on_change(&higher, window, cx)),
        )
}
