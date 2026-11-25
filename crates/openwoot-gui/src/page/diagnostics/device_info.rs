use gpui_kit::{
    App, ClipboardItem, Div, ParentElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        notification::Notification,
        v_flex,
    },
    div, px,
};

use crate::{page::settings::layout_name, store::Keyboard};

pub(in crate::page) type Line = (&'static str, String);

const DIALOG_WIDTH: f32 = 460.0;

pub(in crate::page) fn device_lines(keyboard: &Keyboard) -> Vec<Line> {
    let capabilities = &keyboard.capabilities;
    vec![
        ("Keyboard", keyboard.name.to_string()),
        ("Firmware", keyboard.version.firmware.to_string()),
        ("Protocol", keyboard.version.protocol.to_string()),
        (
            "Layout",
            keyboard.layout.map_or_else(
                || "Unknown".to_owned(),
                |layout| layout_name(layout).to_owned(),
            ),
        ),
        (
            "Layouts",
            capabilities
                .layouts
                .iter()
                .map(|layout| layout_name(*layout))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        (
            "Key matrix",
            format!(
                "{} × {} ({} keys)",
                capabilities.key_rows,
                capabilities.key_columns,
                capabilities.key_count()
            ),
        ),
        (
            "Profiles",
            format!("{} of {}", keyboard.profiles.len(), capabilities.profiles),
        ),
        (
            "Light bar",
            match capabilities.lightbar_leds {
                0 => "None".to_owned(),
                1 => "1 LED".to_owned(),
                leds => format!("{leds} LEDs"),
            },
        ),
        (
            "Host signals",
            if capabilities.host_signals {
                "Yes"
            } else {
                "No"
            }
            .to_owned(),
        ),
        ("Switch models", capabilities.switch_models.to_string()),
        (
            "Edition",
            capabilities
                .edition
                .map_or_else(|| "None".to_owned(), |edition| edition.to_string()),
        ),
    ]
}

pub(in crate::page) fn text_of(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|(label, value)| format!("{label}: {value}\n"))
        .collect::<Vec<_>>()
        .concat()
}

pub(in crate::page) fn value_line((label, value): &Line, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .text_sm()
        .child(div().text_color(theme.foreground).child(*label))
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(value.clone()),
        )
}

pub(in crate::page) fn open_device_info(keyboard: &Keyboard, window: &mut Window, cx: &mut App) {
    let lines = device_lines(keyboard);
    window.open_dialog(cx, move |dialog, _, cx| {
        let text = text_of(&lines);
        dialog
            .title("Device information")
            .w(px(DIALOG_WIDTH))
            .child(
                v_flex()
                    .gap_2()
                    .children(lines.iter().map(|line| value_line(line, cx))),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("copy-device-info")
                            .outline()
                            .label("Copy device info")
                            .on_click(move |_, window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                window.push_notification(Notification::info("Copied"), cx);
                            }),
                    )
                    .child(
                        Button::new("close-device-info")
                            .primary()
                            .label("Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_text_has_one_line_per_entry() {
        let lines = vec![
            ("Keyboard", "Wooting 80HE".to_owned()),
            ("Edition", "None".to_owned()),
        ];
        assert_eq!(text_of(&lines), "Keyboard: Wooting 80HE\nEdition: None\n");
    }
}
