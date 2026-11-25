use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, BoxShadow, Div, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    black,
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
const TILE: f32 = 48.0;
const TILE_BORDER: f32 = 2.0;
const GAP: f32 = 12.0;
const PADDING_TOP: f32 = 20.0;
const PADDING_X: f32 = 20.0;
const PADDING_BOTTOM: f32 = 24.0;
const LABEL_TEXT: f32 = 16.0;
const LABEL_LINE: f32 = 22.4;
const EDIT_GLYPH: f32 = 32.0;
const EDIT_SHADE: f32 = 0.32;
const LIFT_SHADOW: f32 = 0.16;
const LIFT_OFFSET: f32 = -2.4;
const LIFT_BLUR: f32 = 9.6;
const EDITOR_AVATAR: f32 = 80.0;
const EDITOR_RING: f32 = 6.0;
const CARD_AVATAR: f32 = 40.0;
const CARD_RING: f32 = 3.0;
const SMALL_RING: f32 = 2.0;

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
    let ring = if size >= px(EDITOR_AVATAR) {
        px(EDITOR_RING)
    } else if size >= px(CARD_AVATAR) {
        px(CARD_RING)
    } else {
        px(SMALL_RING)
    };
    div()
        .flex_none()
        .size(size)
        .rounded_full()
        .border(ring)
        .border_color(color)
        .bg(cx.theme().sidebar)
        .flex()
        .items_center()
        .justify_center()
        .child(mark(color, icon, size))
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
    let width = px(TILE * columns + GAP * (columns - 1.0) + 2.0 * PADDING_X);
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
                            .rounded_full()
                            .border(px(TILE_BORDER))
                            .border_color(if chosen { theme.primary } else { theme.border })
                            .cursor_pointer()
                            .when(!chosen, |this| {
                                this.hover(move |style| style.border_color(hover))
                            })
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
                .pt(px(PADDING_TOP))
                .px(px(PADDING_X))
                .pb(px(PADDING_BOTTOM))
                .gap(px(GAP))
                .child(
                    div()
                        .text_size(px(LABEL_TEXT))
                        .line_height(px(LABEL_LINE))
                        .text_color(theme.group_box_foreground)
                        .child("ICONS"),
                )
                .children(rows)
                .into_any_element()
        })
        .trigger(move |_, cx| face(color, icon, size, cx))
}

fn face(color: Hsla, icon: u8, size: Pixels, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .id("profile-avatar")
        .group("profile-avatar")
        .relative()
        .size(size)
        .rounded_full()
        .cursor_pointer()
        .shadow(vec![BoxShadow {
            color: black().opacity(LIFT_SHADOW),
            offset: point(px(0.0), px(LIFT_OFFSET)),
            blur_radius: px(LIFT_BLUR),
            spread_radius: px(0.0),
            inset: false,
        }])
        .child(avatar(color, icon, size, cx))
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().opacity(EDIT_SHADE))
                .text_color(theme.foreground)
                .opacity(0.0)
                .group_hover("profile-avatar", |style| style.opacity(1.0))
                .child(Icon::new(Catalog::Pencil).size(px(EDIT_GLYPH))),
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
