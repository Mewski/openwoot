use gpui_kit::{
    AnyElement, App, Div, FontWeight, HighlightStyle, Hsla, ParentElement as _, SharedString,
    Styled as _, StyledText,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

use crate::panel;

const COLUMN_WIDTH: f32 = 580.0;
const TOP_PADDING: f32 = 64.0;
const BOTTOM_PADDING: f32 = 256.0;
const TITLE_SIZE: f32 = 24.0;
const TITLE_LINE: f32 = 28.8;
const TITLE_GAP: f32 = 24.0;
const SECTION_GAP: f32 = 32.0;
const DIVIDER: f32 = 1.0;
const HEADING_SIZE: f32 = 20.0;
const HEADING_LINE: f32 = 24.0;
const HEADING_GAP: f32 = 8.0;
const ENTRY_GAP: f32 = 16.0;
const BODY_SIZE: f32 = 16.0;
const BODY_LINE: f32 = 22.4;
const SMALL_SIZE: f32 = 14.0;
const SMALL_LINE: f32 = 19.6;
const CALLOUT_PADDING: f32 = 12.0;
const CALLOUT_GAP: f32 = 8.0;
const CALLOUT_BADGE: f32 = 36.0;
const CALLOUT_ICON: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::page) enum Tone {
    Info,
    Danger,
}

pub(in crate::page) fn sheet(
    id: &'static str,
    title: impl Into<SharedString>,
    sections: impl IntoIterator<Item = Div>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let mut body = v_flex().w_full().gap(px(SECTION_GAP));
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            body = body.child(div().w_full().h(px(DIVIDER)).bg(theme.border));
        }
        body = body.child(section);
    }
    panel::page().child(panel::scroll_area(
        id,
        v_flex()
            .w_full()
            .pt(px(TOP_PADDING))
            .pb(px(BOTTOM_PADDING))
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(COLUMN_WIDTH))
                    .mx_auto()
                    .gap(px(TITLE_GAP))
                    .child(
                        div()
                            .text_size(px(TITLE_SIZE))
                            .line_height(px(TITLE_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(title.into()),
                    )
                    .child(body),
            ),
    ))
}

pub(in crate::page) fn entry(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap(px(ENTRY_GAP))
        .child(heading_row(title, theme.group_box_foreground, trailing))
        .when_some(description, |this, description| {
            this.child(body_text(description, cx))
        })
}

pub(in crate::page) fn heading(
    title: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    heading_row(title, cx.theme().foreground, trailing)
}

pub(in crate::page) fn body_text(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(BODY_SIZE))
        .line_height(px(BODY_LINE))
        .text_color(cx.theme().group_box_foreground)
        .child(text.into())
}

pub(in crate::page) fn emphasised(text: impl Into<SharedString>, strong: &[&str]) -> StyledText {
    let text: SharedString = text.into();
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let mut ranges: Vec<_> = strong
        .iter()
        .filter_map(|part| text.find(*part).map(|start| start..start + part.len()))
        .collect();
    ranges.sort_by_key(|range| range.start);
    StyledText::new(text).with_highlights(ranges.into_iter().map(|range| (range, bold)))
}

pub(in crate::page) fn callout(tone: Tone, text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    let (icon, color) = match tone {
        Tone::Info => (Catalog::Info, theme.info),
        Tone::Danger => (Catalog::CircleAlert, theme.danger),
    };
    h_flex()
        .w_full()
        .gap(px(CALLOUT_GAP))
        .p(px(CALLOUT_PADDING))
        .rounded(theme.radius)
        .bg(theme.tab_active)
        .child(
            div()
                .flex_none()
                .size(px(CALLOUT_BADGE))
                .rounded_full()
                .bg(theme.secondary)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).size(px(CALLOUT_ICON)).text_color(color)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(SMALL_SIZE))
                .line_height(px(SMALL_LINE))
                .text_color(theme.group_box_foreground)
                .child(text.into()),
        )
}

fn heading_row(title: impl Into<SharedString>, color: Hsla, trailing: Option<AnyElement>) -> Div {
    h_flex()
        .w_full()
        .min_h(px(HEADING_LINE))
        .gap(px(HEADING_GAP))
        .justify_between()
        .child(
            div()
                .min_w_0()
                .text_size(px(HEADING_SIZE))
                .line_height(px(HEADING_LINE))
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(title.into()),
        )
        .children(trailing)
}
