use std::{ops::RangeInclusive, rc::Rc};

use gpui_kit::{
    App, Div, Entity, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputState},
        slider::{Slider, SliderState},
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

const VALUE_WIDTH: f32 = 52.0;
const INPUT_WIDTH: f32 = 60.0;
const UNIT_WIDTH: f32 = 44.0;
const STEP_VALUE_WIDTH: f32 = 64.0;
const CHOICE_HEIGHT: f32 = 34.0;
const TILE_HEIGHT: f32 = 38.0;
const PILL_HEIGHT: f32 = 46.0;
const BANNER_ICON: f32 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Filled,
    Outlined,
}

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
        .child(
            div()
                .flex_none()
                .w(px(VALUE_WIDTH))
                .text_right()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(value.into()),
        )
}

pub(crate) fn dial_row(
    label: impl Into<SharedString>,
    slider: &Entity<SliderState>,
    input: &Entity<InputState>,
    unit: &'static str,
    cx: &App,
) -> Div {
    v_flex()
        .w_full()
        .gap_1()
        .child(param_label(label, cx))
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .items_center()
                .child(div().flex_1().min_w_0().child(Slider::new(slider)))
                .child(
                    div()
                        .flex_none()
                        .w(px(INPUT_WIDTH))
                        .child(Input::new(input).small()),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(UNIT_WIDTH))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(unit),
                ),
        )
}

pub(crate) fn param_label(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().foreground)
        .child(label.into())
}

pub(crate) fn choices(
    id: impl Into<SharedString>,
    labels: impl IntoIterator<Item = impl Into<SharedString>>,
    selected: Option<usize>,
    style: Choice,
    on_pick: impl Fn(&usize, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let id: SharedString = id.into();
    let on_pick = Rc::new(on_pick);
    let theme = cx.theme();
    let (primary, foreground, text, field, idle) = (
        theme.primary,
        theme.primary_foreground,
        theme.foreground,
        theme.background,
        theme.secondary,
    );
    h_flex()
        .flex_wrap()
        .gap_2()
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let chosen = selected == Some(index);
            let on_pick = Rc::clone(&on_pick);
            div()
                .id(SharedString::from(format!("{id}-{index}")))
                .h(px(CHOICE_HEIGHT))
                .px_3()
                .flex()
                .items_center()
                .rounded(px(6.0))
                .border_1()
                .text_sm()
                .cursor_pointer()
                .map(|this| match (style, chosen) {
                    (Choice::Filled, true) => this
                        .bg(primary)
                        .border_color(primary)
                        .text_color(foreground)
                        .font_weight(FontWeight::MEDIUM),
                    (Choice::Outlined, true) => {
                        this.bg(field).border_color(primary).text_color(text)
                    }
                    (_, false) => this
                        .bg(idle)
                        .border_color(theme.transparent)
                        .text_color(text)
                        .hover(move |style| style.border_color(primary)),
                })
                .child(label.into())
                .on_click(move |_, window, cx| on_pick(&index, window, cx))
        }))
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
    let primary = theme.primary;
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(TILE_HEIGHT))
        .px_3()
        .gap_3()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(primary)
                    .border_color(primary)
                    .text_color(theme.primary_foreground)
            } else {
                this.border_color(theme.transparent)
                    .text_color(theme.foreground)
                    .hover(move |style| style.border_color(primary))
            }
        })
        .child(Icon::new(icon).small())
        .child(div().min_w_0().truncate().child(label))
        .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
}

pub(crate) fn pill(
    id: impl Into<SharedString>,
    icon: Option<Catalog>,
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    h_flex()
        .id(id.into())
        .w_full()
        .min_h(px(PILL_HEIGHT))
        .pl_4()
        .pr_2()
        .py_1()
        .gap_3()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.transparent
        })
        .bg(theme.background)
        .cursor_pointer()
        .when(!selected, |this| {
            this.hover(move |style| style.border_color(hover))
        })
        .when_some(icon, |this, icon| this.child(Icon::new(icon).small()))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(label.into()),
                )
                .children(detail.map(|detail| {
                    div()
                        .truncate()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(detail)
                })),
        )
}

pub(crate) fn add_button(id: impl Into<SharedString>, label: &'static str, cx: &App) -> Button {
    let theme = cx.theme();
    Button::new(id.into())
        .icon(IconName::Plus)
        .label(label)
        .ghost()
        .w_full()
        .h(px(PILL_HEIGHT))
        .border_1()
        .border_color(theme.border)
        .text_color(theme.muted_foreground)
}

pub(crate) fn sunken(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap_1()
        .p_2()
        .rounded(px(8.0))
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
}

pub(crate) fn banner(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .gap_3()
        .p_3()
        .items_center()
        .rounded(px(8.0))
        .bg(theme.secondary)
        .child(
            div()
                .flex_none()
                .size(px(BANNER_ICON))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.background)
                .text_color(theme.primary)
                .child(Icon::new(IconName::TriangleAlert).small()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(theme.foreground)
                .child(text.into()),
        )
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

pub(crate) fn kebab(open: bool, cx: &App) -> gpui_kit::AnyElement {
    let theme = cx.theme();
    div()
        .size(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.0))
        .text_color(theme.muted_foreground)
        .when(open, |this| this.bg(theme.secondary))
        .hover(|style| style.bg(theme.secondary))
        .child(Icon::new(IconName::EllipsisVertical).small())
        .into_any_element()
}
