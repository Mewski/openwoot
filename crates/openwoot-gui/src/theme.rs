use std::rc::Rc;

use gpui_kit::{
    App, Hsla,
    component::{Theme, ThemeMode, ThemeSet},
    rgb,
};

const THEMES: &str = include_str!("../assets/theme.json");

#[derive(Debug, Clone, Copy)]
pub(crate) struct Keycaps {
    pub(crate) case: Hsla,
    pub(crate) plate: Hsla,
    pub(crate) side: Hsla,
    pub(crate) top: Hsla,
    pub(crate) legend: Hsla,
    pub(crate) detail: Hsla,
}

pub(crate) fn apply(cx: &mut App) {
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
        case: rgb(0x001a_1a1a).into(),
        plate: rgb(0x002a_2b2d).into(),
        side: rgb(0x000f_1011).into(),
        top: rgb(0x002b_2e31).into(),
        legend: rgb(0x00e8_ebed).into(),
        detail: rgb(0x00a6_adb5).into(),
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
