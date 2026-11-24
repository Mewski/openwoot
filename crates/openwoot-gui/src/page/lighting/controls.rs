use std::{ops::RangeInclusive, rc::Rc};

use gpui_kit::{
    App, Div, Entity, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        slider::{Slider, SliderState},
        tab::{Tab, TabBar},
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

const VALUE_WIDTH: f32 = 52.0;
const STEP_VALUE_WIDTH: f32 = 64.0;

pub(crate) fn wide_slider(
    state: &Entity<SliderState>,
    value: impl Into<SharedString>,
    cx: &App,
) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(div().flex_1().min_w_0().child(Slider::new(state)))
        .child(value_text(value, cx))
}

fn value_text(value: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .flex_none()
        .w(px(VALUE_WIDTH))
        .text_right()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(value.into())
}

pub(crate) fn segmented(
    id: impl Into<SharedString>,
    labels: impl IntoIterator<Item = impl Into<SharedString>>,
    selected: usize,
    on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> TabBar {
    TabBar::new(id.into())
        .segmented()
        .small()
        .selected_index(selected)
        .on_click(on_pick)
        .children(
            labels
                .into_iter()
                .map(|label| Tab::new().label(label.into())),
        )
}

pub(crate) fn select(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Button {
    Button::new(id.into())
        .label(label.into())
        .outline()
        .dropdown_caret(true)
        .w_full()
}

pub(crate) fn pill(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    muted: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    h_flex()
        .id(id.into())
        .w_full()
        .h_9()
        .pl_3()
        .pr_1()
        .gap_2()
        .items_center()
        .rounded(theme.radius)
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.transparent
        })
        .bg(theme.secondary)
        .cursor_pointer()
        .hover(|style| style.bg(theme.secondary_hover))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(if muted {
                    theme.muted_foreground
                } else {
                    theme.foreground
                })
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_xs()
                .text_color(theme.muted_foreground)
                .children(detail),
        )
}

pub(crate) fn tile(
    id: impl Into<SharedString>,
    icon: Catalog,
    label: &'static str,
    description: &'static str,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    h_flex()
        .id(id.into())
        .w_full()
        .h_9()
        .px_3()
        .gap_2()
        .items_center()
        .rounded(theme.radius)
        .text_sm()
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(theme.primary).text_color(theme.primary_foreground)
            } else {
                this.text_color(theme.foreground)
                    .hover(|style| style.bg(theme.secondary))
            }
        })
        .child(Icon::new(icon).small())
        .child(div().min_w_0().truncate().child(label))
        .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
}

pub(crate) fn add_button(id: impl Into<SharedString>, label: &'static str, cx: &App) -> Button {
    let theme = cx.theme();
    Button::new(id.into())
        .icon(IconName::Plus)
        .label(label)
        .ghost()
        .w_full()
        .h_9()
        .border_1()
        .border_dashed()
        .border_color(theme.border)
        .text_color(theme.muted_foreground)
}

pub(crate) fn sunken(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap_1()
        .p_2()
        .rounded(theme.radius)
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
}

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
                .min_w(px(STEP_VALUE_WIDTH))
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
