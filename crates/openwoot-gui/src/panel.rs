use gpui_kit::{
    AnyElement, App, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

pub(crate) const RAIL_WIDTH: f32 = 72.0;
pub(crate) const NAVIGATION_WIDTH: f32 = 256.0;
pub(crate) const NAVIGATION_MARGIN: f32 = 16.0;
const PAGE_WIDTH: f32 = 1240.0;
const PAGE_PADDING: f32 = 24.0;
const SCROLLBAR_ALLOWANCE: f32 = 12.0;
const CONTROL_WIDTH: f32 = 240.0;
const SIDE_WIDTH: f32 = 140.0;
const SIDE_GAP: f32 = 20.0;
const HELP_TOOLTIP_WIDTH: f32 = 360.0;

pub(crate) fn content_width(window: &Window) -> Pixels {
    let shell = RAIL_WIDTH + NAVIGATION_WIDTH + NAVIGATION_MARGIN + SCROLLBAR_ALLOWANCE;
    let page = (f32::from(window.viewport_size().width) - shell).min(PAGE_WIDTH);
    px(page - 2.0 * PAGE_PADDING)
}

pub(crate) fn content_width_beside_side(window: &Window) -> Pixels {
    content_width(window) - px(SIDE_WIDTH + SIDE_GAP)
}

pub(crate) fn page() -> Div {
    v_flex()
        .w_full()
        .max_w(px(PAGE_WIDTH))
        .mx_auto()
        .px_6()
        .pt_4()
        .pb_8()
        .gap_5()
}

pub(crate) fn page_with_stage(
    id: &'static str,
    stage: impl IntoElement,
    body: impl IntoElement,
) -> Div {
    page()
        .h_full()
        .pb_0()
        .child(div().flex_none().child(stage))
        .child(
            div()
                .id(id)
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(v_flex().gap_5().pb_8().child(body)),
        )
}

pub(crate) fn stage(content: impl IntoElement) -> Div {
    h_flex().w_full().justify_center().child(content)
}

pub(crate) fn stage_with_side(side: impl IntoElement, content: impl IntoElement) -> Div {
    h_flex()
        .w_full()
        .items_start()
        .gap_5()
        .child(v_flex().flex_none().w(px(SIDE_WIDTH)).pt_4().child(side))
        .child(h_flex().flex_1().min_w_0().justify_center().child(content))
}

pub(crate) fn heading(
    title: impl Into<SharedString>,
    help: Option<SharedString>,
    trailing: impl IntoElement,
    cx: &App,
) -> Div {
    let title = title.into();
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .child(
            h_flex()
                .gap_2()
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title.clone()),
                )
                .when_some(help, |this, help| this.child(help_icon(&title, help, cx))),
        )
        .child(h_flex().gap_2().child(trailing))
}

pub(crate) fn columns(left: impl IntoElement, right: impl IntoElement) -> Div {
    h_flex()
        .w_full()
        .items_start()
        .gap_4()
        .child(v_flex().flex_1().min_w_0().gap_4().child(left))
        .child(v_flex().flex_1().min_w_0().gap_4().child(right))
}

pub(crate) fn section(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    cx: &App,
) -> Div {
    card(title.into(), None, description, None, cx)
}

pub(crate) fn section_with(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    trailing: impl IntoElement,
    cx: &App,
) -> Div {
    card(
        title.into(),
        None,
        description,
        Some(trailing.into_any_element()),
        cx,
    )
}

pub(crate) fn section_with_help(
    title: impl Into<SharedString>,
    help: impl Into<SharedString>,
    description: Option<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    card(title.into(), Some(help.into()), description, trailing, cx)
}

fn card(
    title: SharedString,
    help: Option<SharedString>,
    description: Option<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let help = help.map(|help| help_icon(&title, help, cx));
    let heading = h_flex()
        .gap_1p5()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(title),
        )
        .children(help);
    v_flex()
        .w_full()
        .gap_4()
        .p_6()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
        .child(
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .gap_3()
                        .justify_between()
                        .child(heading)
                        .children(trailing),
                )
                .when_some(description, |this, description| {
                    this.child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(description),
                    )
                }),
        )
}

pub(crate) fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(SharedString::from(text.into().to_uppercase()))
}

pub(crate) fn field(text: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap_2()
        .child(label(text, cx))
        .child(control)
}

pub(crate) fn row(
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    labelled(&label.into(), help, cx).child(
        h_flex()
            .flex_none()
            .min_w(px(CONTROL_WIDTH))
            .justify_end()
            .child(control),
    )
}

pub(crate) fn toggle(
    label: impl Into<SharedString>,
    help: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    labelled(&label.into(), help, cx).child(h_flex().flex_none().child(control))
}

fn labelled(label: &SharedString, help: Option<SharedString>, cx: &App) -> Div {
    h_flex().w_full().min_h_8().gap_3().justify_between().child(
        h_flex()
            .flex_1()
            .min_w_0()
            .gap_1p5()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(label.clone()),
            )
            .when_some(help, |this, help| this.child(help_icon(label, help, cx))),
    )
}

pub(crate) fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

fn help_icon(owner: &SharedString, help: SharedString, cx: &App) -> AnyElement {
    div()
        .id(SharedString::from(format!("help-{owner}")))
        .text_color(cx.theme().muted_foreground)
        .child(Icon::new(Catalog::CircleQuestionMark).small())
        .tooltip(move |window, cx| {
            Tooltip::new(help.clone())
                .max_w(px(HELP_TOOLTIP_WIDTH))
                .build(window, cx)
        })
        .into_any_element()
}
