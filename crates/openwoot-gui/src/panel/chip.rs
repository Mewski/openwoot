use gpui_kit::{
    App, Div, FontWeight, Hsla, ParentElement as _, SharedString, Styled as _,
    component::ActiveTheme as _, div, px,
};
use openwoot_protocol::keys::Action;

use crate::usage;

const HEIGHT: f32 = 20.0;
const PADDING_X: f32 = 10.0;
const RADIUS: f32 = 8.0;
const TEXT: f32 = 12.8;
const LEFT_CONTROL: u8 = 0xE0;
const LEFT_GUI: u8 = 0xE3;
const SHORTCUT_MODIFIER: u8 = if cfg!(target_os = "macos") {
    LEFT_GUI
} else {
    LEFT_CONTROL
};
const SHIFT: &str = "Shift";
pub(crate) const KBD_GAP: f32 = 4.0;

pub(crate) fn kbd(legend: impl Into<SharedString>, color: Hsla, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .h(px(HEIGHT))
        .px(px(PADDING_X))
        .flex()
        .items_center()
        .rounded(px(RADIUS))
        .bg(theme.secondary)
        .font_family(theme.mono_font_family.clone())
        .text_size(px(TEXT))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .whitespace_nowrap()
        .child(legend.into())
}

pub(crate) fn shortcut(key: u8, shift: bool) -> Vec<SharedString> {
    let legend = |code| SharedString::from(usage::legend(Action::Keyboard(code)));
    let mut keys = vec![legend(SHORTCUT_MODIFIER)];
    if shift {
        keys.push(SHIFT.into());
    }
    keys.push(legend(key));
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_start_with_the_platform_modifier_and_end_with_the_key() {
        let modifier = SharedString::from(usage::legend(Action::Keyboard(SHORTCUT_MODIFIER)));
        assert_eq!(
            shortcut(0x06, false),
            vec![modifier.clone(), SharedString::from("C")]
        );
        assert_eq!(
            shortcut(0x1D, true),
            vec![modifier, SharedString::from(SHIFT), SharedString::from("Z")]
        );
    }
}
