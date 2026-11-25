use gpui_kit::{
    App, ClipboardItem, Div, FontWeight, ParentElement as _, Styled as _, Window,
    component::{ActiveTheme as _, WindowExt as _, h_flex, notification::Notification, v_flex},
    div, px,
};

use crate::{page::settings::layout_name, panel::ActionButton, store::Keyboard};

type Line = (&'static str, String);

const DIALOG_WIDTH: f32 = 512.0;
const LINE_GAP: f32 = 4.0;
const GRID_GAP: f32 = 16.0;
const LABEL_SIZE: f32 = 14.0;
const VALUE_SIZE: f32 = 16.0;

pub(super) fn open_device_info(keyboard: &Keyboard, window: &mut Window, cx: &mut App) {
    let lines = device_lines(keyboard);
    window.open_dialog(cx, move |dialog, _, cx| {
        let text = text_of(&lines);
        let (first, second) = lines.split_at(lines.len().div_ceil(2));
        let theme = cx.theme();
        dialog
            .title("Device information")
            .w(px(DIALOG_WIDTH))
            .centered()
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(GRID_GAP))
                    .p(px(GRID_GAP))
                    .rounded(theme.radius)
                    .bg(theme.accent)
                    .child(column(first, cx))
                    .child(column(second, cx)),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        ActionButton::new("copy-device-info", "Copy device info").on_click(
                            move |_, window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                window.push_notification(Notification::info("Copied"), cx);
                            },
                        ),
                    )
                    .child(
                        ActionButton::new("close-device-info", "Close")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
            )
    });
}

fn device_lines(keyboard: &Keyboard) -> Vec<Line> {
    let capabilities = &keyboard.capabilities;
    vec![
        ("Device name", keyboard.display_name().to_string()),
        ("Model", keyboard.name.to_string()),
        ("Firmware", keyboard.version.firmware.to_string()),
        ("Status", "Connected".to_owned()),
        (
            "Layout",
            keyboard.layout.map_or("Unknown", layout_name).to_owned(),
        ),
        ("Protocol", keyboard.version.protocol.to_string()),
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
    ]
}

fn text_of(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|(label, value)| format!("{label}: {value}\n"))
        .collect::<Vec<_>>()
        .concat()
}

fn column(lines: &[Line], cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(GRID_GAP))
        .children(lines.iter().map(|(label, value)| {
            v_flex()
                .gap(px(LINE_GAP))
                .child(
                    div()
                        .text_size(px(LABEL_SIZE))
                        .text_color(theme.group_box_foreground)
                        .child(*label),
                )
                .child(
                    div()
                        .text_size(px(VALUE_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(value.clone()),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_text_has_one_line_per_entry() {
        let lines = vec![
            ("Model", "Wooting 80HE".to_owned()),
            ("Status", "Connected".to_owned()),
        ];
        assert_eq!(text_of(&lines), "Model: Wooting 80HE\nStatus: Connected\n");
    }
}
