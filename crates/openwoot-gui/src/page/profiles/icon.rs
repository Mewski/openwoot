use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, BoxShadow, Div, ElementId, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div, point,
    prelude::FluentBuilder as _,
    px,
};

use crate::dropdown::Dropdown;

const ICONS: [Option<Catalog>; 24] = [
    None,
    Some(Catalog::Crosshair),
    Some(Catalog::Gamepad2),
    Some(Catalog::Joystick),
    Some(Catalog::Keyboard),
    Some(Catalog::Car),
    Some(Catalog::Gauge),
    Some(Catalog::Music),
    Some(Catalog::Headphones),
    Some(Catalog::Code),
    Some(Catalog::Monitor),
    Some(Catalog::Briefcase),
    Some(Catalog::Swords),
    Some(Catalog::Target),
    Some(Catalog::Trophy),
    Some(Catalog::Zap),
    Some(Catalog::Flame),
    Some(Catalog::Rocket),
    Some(Catalog::Star),
    Some(Catalog::Heart),
    Some(Catalog::Ghost),
    Some(Catalog::Skull),
    Some(Catalog::Moon),
    Some(Catalog::Coffee),
];
const PER_ROW: usize = 6;
const TILE: f32 = 40.0;
const GAP: f32 = 6.0;
const PADDING: f32 = 12.0;
const BADGE: f32 = 26.0;

pub(super) fn index_of(icon: Catalog) -> u8 {
    ICONS
        .iter()
        .position(|entry| *entry == Some(icon))
        .and_then(|index| u8::try_from(index).ok())
        .unwrap_or_default()
}

fn glyph(icon: u8) -> Option<Catalog> {
    ICONS.get(usize::from(icon)).copied().flatten()
}

pub(crate) fn avatar(color: Hsla, icon: u8, size: Pixels, cx: &App) -> Div {
    let theme = cx.theme();
    let ring = div()
        .flex_none()
        .size(size)
        .rounded_full()
        .border_color(color)
        .bg(theme.sidebar)
        .shadow(vec![BoxShadow {
            color: color.opacity(0.45),
            offset: point(px(0.0), px(0.0)),
            blur_radius: size / 4.0,
            spread_radius: px(0.0),
            inset: false,
        }])
        .flex()
        .items_center()
        .justify_center()
        .child(mark(color, icon, size));
    if size >= px(64.0) {
        ring.border_4()
    } else {
        ring.border_2()
    }
}

fn mark(color: Hsla, icon: u8, size: Pixels) -> AnyElement {
    match glyph(icon) {
        Some(icon) => Icon::new(icon)
            .size(size * 0.45)
            .text_color(color)
            .into_any_element(),
        None => div()
            .size(size / 3.0)
            .rounded_full()
            .bg(color)
            .into_any_element(),
    }
}

pub(super) fn picker(
    id: impl Into<ElementId>,
    color: Hsla,
    icon: u8,
    size: Pixels,
    on_pick: impl Fn(u8, &mut Window, &mut App) + 'static,
) -> Dropdown {
    let on_pick = Rc::new(on_pick);
    let columns = f32::from(u8::try_from(PER_ROW).unwrap_or_default());
    let width = px(TILE * columns + GAP * (columns - 1.0) + 2.0 * PADDING);
    Dropdown::new(id)
        .menu_width(width)
        .content(move |close, _, cx| {
            let theme = cx.theme();
            let rows = ICONS.chunks(PER_ROW).enumerate().map(|(row, entries)| {
                h_flex()
                    .gap(px(GAP))
                    .children(entries.iter().enumerate().map(|(column, entry)| {
                        let index = row * PER_ROW + column;
                        let value = u8::try_from(index).unwrap_or_default();
                        let chosen = value == icon;
                        let close = close.clone();
                        let on_pick = Rc::clone(&on_pick);
                        let hover = theme.secondary_hover;
                        div()
                            .id(("profile-icon", index))
                            .size(px(TILE))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(theme.radius)
                            .bg(theme.secondary)
                            .border_1()
                            .border_color(if chosen {
                                theme.primary
                            } else {
                                theme.secondary
                            })
                            .cursor_pointer()
                            .when(!chosen, |this| this.hover(move |style| style.bg(hover)))
                            .on_click(move |_, window, cx| {
                                on_pick(value, window, cx);
                                close.close(cx);
                            })
                            .child(match entry {
                                Some(entry) => Icon::new(*entry)
                                    .size(px(TILE * 0.5))
                                    .text_color(theme.foreground)
                                    .into_any_element(),
                                None => div()
                                    .size(px(TILE * 0.3))
                                    .rounded_full()
                                    .bg(theme.foreground)
                                    .into_any_element(),
                            })
                    }))
            });
            v_flex()
                .p(px(PADDING))
                .gap(px(GAP))
                .child(
                    div()
                        .pb_1()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.muted_foreground)
                        .child("PROFILE ICON"),
                )
                .children(rows)
                .into_any_element()
        })
        .trigger(move |open, cx| face(color, icon, size, open, cx))
}

fn face(color: Hsla, icon: u8, size: Pixels, open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .relative()
        .cursor_pointer()
        .child(avatar(color, icon, size, cx))
        .child(
            div()
                .absolute()
                .right(px(-2.0))
                .bottom(px(-2.0))
                .size(px(BADGE))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(if open { theme.primary } else { theme.secondary })
                .border_2()
                .border_color(theme.background)
                .text_color(if open {
                    theme.primary_foreground
                } else {
                    theme.foreground
                })
                .child(Icon::new(Catalog::Pencil).size(px(BADGE * 0.5))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_icon_is_the_plain_dot() {
        assert_eq!(glyph(0), None);
        assert_eq!(glyph(1), Some(Catalog::Crosshair));
        assert_eq!(glyph(u8::MAX), None);
    }

    #[test]
    fn template_icons_map_to_their_index() {
        assert_eq!(index_of(Catalog::Crosshair), 1);
        assert_eq!(index_of(Catalog::Keyboard), 4);
        assert_eq!(index_of(Catalog::Plus), 0);
    }
}
