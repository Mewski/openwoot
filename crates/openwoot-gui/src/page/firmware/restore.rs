use gpui_kit::{
    App, Div, FontWeight, ParentElement as _, Styled as _,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
};

use crate::page::settings::entry;

const STEPS: [(&str, &str); 3] = [
    (
        "Hold the keys and plug in",
        "Unplug the keyboard, hold the top-right and bottom-right keys fully down, and plug it \
         back in.",
    ),
    (
        "Check the keys",
        "The two keys glow blue while the keyboard waits in update mode.",
    ),
    (
        "Install a firmware file",
        "Choose or drop a file above; the update finds the keyboard in update mode.",
    ),
];

pub(super) fn section(cx: &App) -> Div {
    entry(
        "Restore",
        Some("When the keyboard does not start, install the firmware from update mode.".into()),
        None,
        cx,
    )
    .child(
        h_flex().w_full().items_stretch().gap_3().children(
            STEPS
                .iter()
                .enumerate()
                .map(|(index, (title, text))| step(index + 1, title, text, cx)),
        ),
    )
}

fn step(number: usize, title: &'static str, text: &'static str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_2()
        .p_4()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
        .child(
            h_flex()
                .gap_2()
                .child(
                    div()
                        .flex_none()
                        .size_5()
                        .rounded_full()
                        .bg(theme.primary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.primary_foreground)
                        .child(number.to_string()),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text),
        )
}
