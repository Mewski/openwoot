use gpui_kit::{
    AnyElement, App, BoxShadow, Div, Entity, Focusable as _, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, h_flex,
        input::{Input, InputState},
        v_flex,
    },
    div, hsla, point,
    prelude::FluentBuilder as _,
    px,
};

use super::tip::{Tip, with_tip};
use crate::theme;

const PADDING: f32 = 24.0;
const RADIUS: f32 = 8.0;
const HEADER_HEIGHT: f32 = 24.0;
const HEADER_GAP: f32 = 16.0;
const BODY_GAP: f32 = 24.0;
const TITLE_GAP: f32 = 8.0;
const TITLE_TEXT: f32 = 20.0;
const TITLE_LINE: f32 = 24.0;
const BODY_TEXT: f32 = 16.0;
const BODY_LINE: f32 = 22.4;
const LABEL_TEXT: f32 = 14.0;
const LABEL_LINE: f32 = 16.8;
const LABEL_GAP: f32 = 8.0;
const TOGGLE_HEIGHT: f32 = 24.0;
const TOGGLE_LABEL_GAP: f32 = 4.0;
const DESCRIBED_HEIGHT: f32 = 32.0;
const DESCRIBED_TITLE_LINE: f32 = 19.2;
const CONTROL_WIDTH: f32 = 240.0;
const HELP_ICON: f32 = 20.0;
const INFO_PADDING: f32 = 12.0;
const INFO_GAP: f32 = 8.0;
const INFO_DISC: f32 = 36.0;
const COMPACT_PADDING_X: f32 = 12.0;
const COMPACT_PADDING_Y: f32 = 8.0;
const COMPACT_DISC: f32 = 24.0;
const COMPACT_ICON: f32 = 16.0;
const INFO_TEXT: f32 = 14.0;
const INFO_LINE: f32 = 19.6;
const INFO_TITLE_LINE: f32 = 24.0;
const INFO_TITLE_GAP: f32 = 4.0;
const TEXT_FIELD_HEIGHT: f32 = 40.0;
const TEXT_FIELD_RADIUS: f32 = 6.0;
const TEXT_FIELD_PADDING: f32 = 8.0;
const PLACEHOLDER_ALPHA: f32 = 0.24;

#[derive(IntoElement)]
struct HelpIcon {
    owner: SharedString,
    text: SharedString,
}

pub(crate) fn section(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    cx: &App,
) -> Div {
    card(title.into(), None, description, None, None, cx)
}

pub(crate) fn section_with_inline(
    title: impl Into<SharedString>,
    inline: impl IntoElement,
    cx: &App,
) -> Div {
    card(
        title.into(),
        None,
        None,
        None,
        Some(inline.into_any_element()),
        cx,
    )
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
        None,
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
    card(
        title.into(),
        Some(help.into()),
        description,
        trailing,
        None,
        cx,
    )
}

pub(crate) fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().group_box_foreground)
        .child(SharedString::from(text.into().to_uppercase()))
}

pub(crate) fn field(text: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap(px(LABEL_GAP))
        .child(label(text, cx))
        .child(control)
}

pub(crate) fn field_with_help(
    text: impl Into<SharedString>,
    help_text: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    let text = text.into();
    v_flex()
        .w_full()
        .gap(px(LABEL_GAP))
        .child(
            h_flex()
                .gap(px(TITLE_GAP))
                .child(label(text.clone(), cx))
                .child(help(&text, help_text.into(), cx)),
        )
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

pub(crate) fn toggle_described(
    title: impl Into<SharedString>,
    text: impl Into<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap(px(LABEL_GAP))
        .child(
            h_flex()
                .w_full()
                .h(px(DESCRIBED_HEIGHT))
                .gap(px(TITLE_GAP))
                .justify_between()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(BODY_TEXT))
                        .line_height(px(DESCRIBED_TITLE_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.group_box_foreground)
                        .child(title.into()),
                )
                .child(h_flex().flex_none().child(control)),
        )
        .child(description(text, cx))
}

pub(crate) fn text_field(
    state: &Entity<InputState>,
    placeholder: impl Into<SharedString>,
    window: &Window,
    cx: &App,
) -> Div {
    let input = theme::palette().input;
    let theme = cx.theme();
    let focused = state.focus_handle(cx).is_focused(window);
    let empty = state.read(cx).value().is_empty();
    let hover = theme.secondary_hover;
    div()
        .relative()
        .w_full()
        .h(px(TEXT_FIELD_HEIGHT))
        .rounded(px(TEXT_FIELD_RADIUS))
        .bg(input)
        .border_1()
        .border_color(if focused { theme.primary } else { theme.border })
        .when(!focused, |this| {
            this.hover(move |style| style.border_color(hover))
        })
        .when(empty, |this| {
            this.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(TEXT_FIELD_PADDING - 1.0))
                    .flex()
                    .items_center()
                    .text_size(px(BODY_TEXT))
                    .text_color(hsla(0.0, 0.0, 1.0, PLACEHOLDER_ALPHA))
                    .child(placeholder.into()),
            )
        })
        .child(
            Input::new(state)
                .large()
                .appearance(false)
                .h_full()
                .px(px(TEXT_FIELD_PADDING - 1.0))
                .text_color(theme.group_box_foreground),
        )
}

pub(crate) fn note(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(INFO_TEXT))
        .line_height(px(INFO_LINE))
        .text_color(cx.theme().sidebar_foreground)
        .child(text.into())
}

pub(crate) fn info(text: impl Into<SharedString>, cx: &App) -> Div {
    info_box(None, text.into(), false, cx)
}

pub(crate) fn info_compact(text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .px(px(COMPACT_PADDING_X))
        .py(px(COMPACT_PADDING_Y))
        .gap(px(INFO_GAP))
        .rounded(px(RADIUS))
        .bg(theme.tab_active)
        .child(
            div()
                .flex_none()
                .size(px(COMPACT_DISC))
                .rounded_full()
                .bg(theme.secondary)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Info)
                        .size(px(COMPACT_ICON))
                        .text_color(theme.info),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(INFO_TEXT))
                .line_height(px(INFO_LINE))
                .text_color(theme.group_box_foreground)
                .child(text.into()),
        )
}

pub(crate) fn info_titled(
    title: impl Into<SharedString>,
    text: impl Into<SharedString>,
    cx: &App,
) -> Div {
    info_box(Some(title.into()), text.into(), false, cx)
}

pub(crate) fn warning(text: impl Into<SharedString>, cx: &App) -> Div {
    info_box(None, text.into(), true, cx)
}

fn info_box(title: Option<SharedString>, text: SharedString, warning: bool, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .items_start()
        .p(px(INFO_PADDING))
        .gap(px(INFO_GAP))
        .rounded(px(RADIUS))
        .bg(theme.tab_active)
        .child(
            div()
                .flex_none()
                .size(px(INFO_DISC))
                .rounded_full()
                .bg(theme.secondary)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(IconName::Info)
                        .size(px(HELP_ICON))
                        .text_color(if warning { theme.warning } else { theme.info }),
                ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .min_h(px(INFO_DISC))
                .justify_center()
                .gap(px(INFO_TITLE_GAP))
                .text_size(px(INFO_TEXT))
                .when_some(title, |this, title| {
                    this.child(
                        div()
                            .line_height(px(INFO_TITLE_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(title),
                    )
                })
                .child(
                    div()
                        .line_height(px(INFO_LINE))
                        .text_color(theme.group_box_foreground)
                        .child(text),
                ),
        )
}

pub(crate) fn help(owner: &SharedString, text: SharedString, _: &App) -> AnyElement {
    HelpIcon {
        owner: owner.clone(),
        text,
    }
    .into_any_element()
}

pub(crate) fn description(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .w_full()
        .text_size(px(BODY_TEXT))
        .line_height(px(BODY_LINE))
        .text_color(cx.theme().group_box_foreground)
        .child(text.into())
}

fn shadow() -> Vec<BoxShadow> {
    [(0.1, 3.0), (0.06, 2.0)]
        .into_iter()
        .map(|(alpha, blur)| BoxShadow {
            color: hsla(0.0, 0.0, 0.0, alpha),
            offset: point(px(0.0), px(1.0)),
            blur_radius: px(blur),
            spread_radius: px(0.0),
            inset: false,
        })
        .collect()
}

impl RenderOnce for HelpIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = SharedString::from(format!("help-{}", self.owner));
        let theme = cx.theme();
        let hover = theme.primary;
        let element = div()
            .id(id.clone())
            .flex_none()
            .text_color(theme.muted_foreground)
            .hover(move |style| style.text_color(hover))
            .child(Icon::new(Catalog::CircleQuestionMark).size(px(HELP_ICON)));
        with_tip(
            element,
            id,
            Tip::Help {
                title: self.owner,
                text: self.text,
            },
            window,
            cx,
        )
    }
}

fn card(
    title: SharedString,
    help_text: Option<SharedString>,
    summary: Option<SharedString>,
    trailing: Option<AnyElement>,
    inline: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let help = help_text.map(|text| help(&title, text, cx));
    let header = h_flex()
        .w_full()
        .h(px(HEADER_HEIGHT))
        .gap(px(TITLE_GAP))
        .justify_between()
        .child(
            h_flex()
                .min_w_0()
                .gap(px(TITLE_GAP))
                .child(
                    div()
                        .truncate()
                        .text_size(px(TITLE_TEXT))
                        .line_height(px(TITLE_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title),
                )
                .children(help)
                .children(inline),
        )
        .children(trailing);
    let described = summary.is_some();
    v_flex()
        .w_full()
        .p(px(PADDING))
        .gap(px(if described { BODY_GAP } else { HEADER_GAP }))
        .rounded(px(RADIUS))
        .bg(theme.group_box)
        .shadow(shadow())
        .child(
            v_flex()
                .w_full()
                .gap(px(HEADER_GAP))
                .child(header)
                .when_some(summary, |this, summary| {
                    this.child(description(summary, cx))
                }),
        )
}

fn labelled(label: &SharedString, help_text: Option<SharedString>, cx: &App) -> Div {
    h_flex()
        .w_full()
        .min_h(px(TOGGLE_HEIGHT))
        .gap(px(TITLE_GAP))
        .justify_between()
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(TOGGLE_LABEL_GAP))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(LABEL_TEXT))
                        .line_height(px(LABEL_LINE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(label.clone()),
                )
                .when_some(help_text, |this, text| this.child(help(label, text, cx))),
        )
}
