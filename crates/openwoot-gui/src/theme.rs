use std::{borrow::Cow, rc::Rc};

use gpui_kit::{
    App, Hsla,
    component::{Theme, ThemeMode, ThemeSet},
    rgb, rgba,
};

const THEMES: &str = include_str!("../assets/theme.json");
const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/NunitoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NunitoSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/NunitoSans-Bold.ttf"),
    include_bytes!("../assets/fonts/NunitoSans-ExtraBold.ttf"),
    include_bytes!("../assets/fonts/NunitoSans-Black.ttf"),
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct Keycaps {
    pub(crate) case_top: Hsla,
    pub(crate) case_bottom: Hsla,
    pub(crate) rim: Hsla,
    pub(crate) lip: Hsla,
    pub(crate) top: Hsla,
    pub(crate) border: Hsla,
    pub(crate) border_hover: Hsla,
    pub(crate) legend: Hsla,
    pub(crate) detail: Hsla,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) border_strong: Hsla,
    pub(crate) raised: Hsla,
    pub(crate) raised_hover: Hsla,
    pub(crate) overlay_hover: Hsla,
    pub(crate) input: Hsla,
    pub(crate) disabled_thumb: Hsla,
    pub(crate) heading: Hsla,
    pub(crate) tooltip: Hsla,
    pub(crate) tint: Hsla,
    pub(crate) ghost_hover: Hsla,
    pub(crate) ghost_active: Hsla,
}

pub(crate) fn apply(cx: &mut App) {
    cx.text_system()
        .add_fonts(FONTS.into_iter().map(Cow::Borrowed).collect())
        .ok();
    if let Some(config) = serde_json::from_str::<ThemeSet>(THEMES)
        .ok()
        .and_then(|set| set.themes.into_iter().next())
    {
        Theme::update(cx, |theme| theme.apply_config(&Rc::new(config)));
    }
    Theme::change(ThemeMode::Dark, None, cx);
}

pub(crate) fn keycaps() -> Keycaps {
    Keycaps {
        case_top: rgb(0x0027_292d).into(),
        case_bottom: rgb(0x001f_2125).into(),
        rim: rgba(0xffff_ff14).into(),
        lip: rgb(0x000b_0d0e).into(),
        top: rgb(0x002b_2e31).into(),
        border: rgb(0x0039_3d41).into(),
        border_hover: rgb(0x005f_676d).into(),
        legend: rgb(0x00db_dbdb).into(),
        detail: rgb(0x00a6_adb5).into(),
    }
}

pub(crate) fn palette() -> Palette {
    Palette {
        border_strong: rgb(0x004c_5257).into(),
        raised: rgb(0x0039_3d41).into(),
        raised_hover: rgb(0x005f_676d).into(),
        overlay_hover: rgb(0x0034_383c).into(),
        input: rgb(0x0011_1213).into(),
        disabled_thumb: rgb(0x0066_6666).into(),
        heading: rgb(0x00ff_ffff).into(),
        tooltip: rgb(0x0039_3d41).into(),
        tint: rgba(0xffff_ff0a).into(),
        ghost_hover: rgba(0xffff_ff14).into(),
        ghost_active: rgba(0xffff_ff29).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_theme_parses_as_one_dark_theme() {
        let set: ThemeSet = serde_json::from_str(THEMES).unwrap();
        assert_eq!(set.themes.len(), 1);
        assert!(set.themes.iter().all(|theme| theme.mode.is_dark()));
    }
}
