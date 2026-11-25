use std::{ops::RangeInclusive, rc::Rc};

use gpui_kit::{
    AnyElement, App, Div, Entity, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, h_flex,
        input::{Input, InputState},
        slider::SliderState,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::{panel::ActionButton, slider::Slider, theme};

pub(crate) const ICON_SIZE: f32 = 20.0;
pub(crate) const LABEL_TEXT: f32 = 14.0;
pub(crate) const PARAM_TEXT: f32 = 12.8;
pub(crate) const CONTROL_HEIGHT: f32 = 32.0;
const LABEL_LINE: f32 = 16.8;
const PARAM_LINE: f32 = 19.2;
const BODY_LINE: f32 = 21.0;
const INPUT_WIDTH: f32 = 48.0;
const FILLED_RADIUS: f32 = 4.0;
const FILLED_GAP: f32 = 12.0;
const OUTLINED_HEIGHT: f32 = 37.0;
const TAB_TEXT: f32 = 16.0;
const TAB_LINE: f32 = 24.0;
const TILE_HEIGHT: f32 = 38.0;
const TILE_RADIUS: f32 = 4.0;
const PILL_HEIGHT: f32 = 48.0;
const PILL_GAP: f32 = 16.0;
const BOX_RADIUS: f32 = 8.0;
const FIELD_RADIUS: f32 = 6.0;
const REMOVE_OPACITY: f32 = 0.25;
const STEP_VALUE_WIDTH: f32 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Choice {
    Filled,
    Outlined,
    Tab,
}

pub(crate) fn param_label(label: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .pr_2()
        .text_size(px(PARAM_TEXT))
        .line_height(px(PARAM_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().group_box_foreground)
        .child(label.into())
}

pub(crate) fn wide_slider(
    state: &Entity<SliderState>,
    value: impl Into<SharedString>,
    disabled: bool,
    cx: &App,
) -> Div {
    h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Slider::new(state).disabled(disabled)),
        )
        .child(
            div()
                .flex_none()
                .px_2()
                .text_size(px(LABEL_TEXT))
                .line_height(px(LABEL_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().group_box_foreground)
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
        .child(param_label(label, cx))
        .child(value_row(slider, input, unit, cx))
}

pub(crate) fn value_row(
    slider: &Entity<SliderState>,
    input: &Entity<InputState>,
    unit: &'static str,
    cx: &App,
) -> Div {
    h_flex()
        .w_full()
        .h(px(CONTROL_HEIGHT))
        .gap_2()
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
                .text_size(px(PARAM_TEXT))
                .line_height(px(PARAM_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(unit),
        )
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
    let primary = theme.primary;
    h_flex()
        .flex_wrap()
        .map(|this| match style {
            Choice::Filled => this.gap(px(FILLED_GAP)),
            Choice::Outlined | Choice::Tab => this.gap_2(),
        })
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let chosen = selected == Some(index);
            let on_pick = Rc::clone(&on_pick);
            div()
                .id(SharedString::from(format!("{id}-{index}")))
                .flex()
                .items_center()
                .px_3()
                .border_1()
                .cursor_pointer()
                .map(|this| match style {
                    Choice::Filled => this
                        .h(px(CONTROL_HEIGHT))
                        .rounded(px(FILLED_RADIUS))
                        .text_size(px(LABEL_TEXT))
                        .font_weight(FontWeight::SEMIBOLD),
                    Choice::Outlined => this
                        .h(px(OUTLINED_HEIGHT))
                        .rounded(px(FIELD_RADIUS))
                        .text_size(px(LABEL_TEXT))
                        .line_height(px(BODY_LINE))
                        .font_weight(FontWeight::SEMIBOLD),
                    Choice::Tab => this
                        .h(px(CONTROL_HEIGHT))
                        .rounded(px(FIELD_RADIUS))
                        .text_size(px(TAB_TEXT))
                        .line_height(px(TAB_LINE)),
                })
                .map(|this| match (style, chosen) {
                    (Choice::Filled, true) => this
                        .bg(primary)
                        .border_color(primary)
                        .text_color(theme.primary_foreground),
                    (Choice::Filled, false) => this
                        .border_color(theme.border)
                        .text_color(theme.description_list_label_foreground)
                        .hover(move |style| style.border_color(primary)),
                    (Choice::Outlined | Choice::Tab, true) => this
                        .bg(theme.secondary)
                        .border_color(primary)
                        .text_color(theme.foreground),
                    (Choice::Outlined, false) => this
                        .bg(theme.accent)
                        .border_color(theme.transparent)
                        .text_color(theme.foreground)
                        .hover(move |style| style.border_color(primary)),
                    (Choice::Tab, false) => this
                        .bg(theme.accent)
                        .border_color(theme.border)
                        .text_color(theme.group_box_foreground)
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
        .px_4()
        .gap_3()
        .items_center()
        .rounded(px(TILE_RADIUS))
        .border_1()
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.bg(primary)
                    .border_color(primary)
                    .text_color(theme.primary_foreground)
            } else {
                this.border_color(theme.transparent)
                    .text_color(theme.description_list_label_foreground)
                    .hover(move |style| style.border_color(primary))
            }
        })
        .child(Icon::new(icon).size(px(ICON_SIZE)))
        .child(div().min_w_0().truncate().child(label))
        .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
}

pub(crate) fn tiles_box(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .w_full()
        .px_2()
        .py_3()
        .rounded(px(BOX_RADIUS))
        .bg(theme::palette().input)
        .border_1()
        .border_color(theme.border)
}

pub(crate) fn pill(
    id: impl Into<SharedString>,
    icon: Catalog,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(PILL_HEIGHT))
        .px_4()
        .gap(px(PILL_GAP))
        .items_center()
        .rounded(px(BOX_RADIUS))
        .border_1()
        .bg(theme::palette().input)
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .map(|this| {
            if selected {
                this.border_color(theme.primary)
                    .text_color(theme.foreground)
            } else {
                this.border_color(theme.border)
                    .text_color(theme.description_list_label_foreground)
                    .hover(move |style| style.border_color(hover))
            }
        })
        .child(Icon::new(icon).size(px(ICON_SIZE)))
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
}

pub(crate) fn remove_button(id: impl Into<SharedString>) -> ActionButton {
    ActionButton::icon_only(id.into(), Icon::new(IconName::Close))
        .tiny()
        .opacity(REMOVE_OPACITY)
}

pub(crate) fn add_button(
    id: impl Into<SharedString>,
    label: &'static str,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let faint = muted.opacity(0.5);
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(PILL_HEIGHT))
        .px_4()
        .gap_2()
        .items_center()
        .rounded(px(BOX_RADIUS))
        .border_1()
        .border_color(faint)
        .bg(theme::palette().input.opacity(0.5))
        .text_size(px(PARAM_TEXT))
        .line_height(px(PARAM_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(muted)
        .cursor_pointer()
        .hover(move |style| style.border_color(muted))
        .child(Icon::new(IconName::Plus).size(px(ICON_SIZE)))
        .child(label)
}

pub(crate) fn sunken(cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .p_4()
        .rounded(px(BOX_RADIUS))
        .bg(theme::palette().input)
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
            ActionButton::icon_only(
                SharedString::from(format!("{id}-lower")),
                Icon::new(IconName::Minus),
            )
            .tiny()
            .disabled(value <= min)
            .on_click(move |_, window, cx| on_lower(&lower, window, cx)),
        )
        .child(
            div()
                .min_w(px(STEP_VALUE_WIDTH))
                .text_center()
                .text_size(px(LABEL_TEXT))
                .child(label.into()),
        )
        .child(
            ActionButton::icon_only(
                SharedString::from(format!("{id}-higher")),
                Icon::new(IconName::Plus),
            )
            .tiny()
            .disabled(value >= max)
            .on_click(move |_, window, cx| on_change(&higher, window, cx)),
        )
}

pub(crate) fn kebab(open: bool, _: &App) -> AnyElement {
    ActionButton::icon_only("kebab", Icon::new(IconName::EllipsisVertical))
        .plain()
        .selected(open)
        .into_any_element()
}
