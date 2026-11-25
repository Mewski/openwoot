use gpui_kit::{
    AnyElement, App, Div, FontWeight, ParentElement as _, SharedString, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

const COLUMN_WIDTH: f32 = 620.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::page) enum Tone {
    Info,
    Danger,
}

pub(in crate::page) fn sheet(
    title: impl Into<SharedString>,
    sections: impl IntoIterator<Item = Div>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let mut column = v_flex()
        .w_full()
        .max_w(px(COLUMN_WIDTH))
        .mx_auto()
        .pt_6()
        .pb_12()
        .gap_8()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(title.into()),
        );
    for (index, section) in sections.into_iter().enumerate() {
        if index > 0 {
            column = column.child(div().w_full().h(px(1.0)).bg(theme.border));
        }
        column = column.child(section);
    }
    column
}

pub(in crate::page) fn entry(
    title: impl Into<SharedString>,
    description: Option<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex().w_full().gap_4().child(
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .min_h_8()
                    .gap_4()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(title.into()),
                    )
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

pub(in crate::page) fn callout(tone: Tone, text: impl Into<SharedString>, cx: &App) -> Div {
    let theme = cx.theme();
    let (icon, color) = match tone {
        Tone::Info => (Catalog::Info, theme.info),
        Tone::Danger => (Catalog::CircleAlert, theme.danger),
    };
    h_flex()
        .w_full()
        .gap_3()
        .p_3()
        .rounded(theme.radius)
        .bg(theme.group_box)
        .child(
            div()
                .flex_none()
                .size_8()
                .rounded_full()
                .bg(theme.background)
                .flex()
                .items_center()
                .justify_center()
                .child(Icon::new(icon).small().text_color(color)),
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
