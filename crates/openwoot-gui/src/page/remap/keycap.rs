use gpui_kit::{
    AnyElement, App, Div, ElementId, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Pixels, SharedString, Stateful, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::keys::{Action, Function, MouseButton, SystemControl};

use crate::{keyboard::Glyph, theme, usage};

pub(crate) const TILE: f32 = 44.0;
const CHIP: f32 = 26.0;
const ICON_SHARE: f32 = 0.42;
const TILE_RADIUS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Art {
    Icon(Catalog),
    Signed(Catalog, &'static str),
    Pair(Catalog, Catalog),
    Mouse(MouseButton),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Look {
    pub(crate) current: bool,
    pub(crate) disabled: bool,
}

pub(crate) fn glyph(action: Action, ink: Hsla, accent: Hsla) -> Option<Glyph> {
    let art = art(action)?;
    Some(Glyph::new(move |height| {
        draw(art, height * ICON_SHARE, ink, accent)
    }))
}

pub(crate) fn alert(color: Hsla) -> Glyph {
    Glyph::new(move |height| {
        Icon::new(Catalog::TriangleAlert)
            .size(height * ICON_SHARE)
            .text_color(color)
            .into_any_element()
    })
}

pub(crate) fn tile(
    id: impl Into<ElementId>,
    action: Action,
    look: Look,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let caps = theme::keycaps();
    let hover = theme.secondary_hover;
    div()
        .id(id)
        .flex_none()
        .size(px(TILE))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded(px(TILE_RADIUS))
        .bg(caps.top)
        .border_1()
        .border_color(if look.current {
            theme.primary
        } else {
            theme.border
        })
        .shadow_sm()
        .text_xs()
        .text_center()
        .text_color(caps.legend)
        .map(|this| {
            if look.disabled {
                this.opacity(0.4)
            } else {
                this.cursor_pointer().hover(move |style| style.bg(hover))
            }
        })
        .child(face(action, px(TILE), caps.legend, theme.primary))
}

pub(crate) fn chip(action: Action, cx: &App) -> Div {
    let theme = cx.theme();
    let caps = theme::keycaps();
    h_flex()
        .flex_none()
        .h(px(CHIP))
        .min_w(px(CHIP))
        .px_1p5()
        .justify_center()
        .rounded(px(TILE_RADIUS))
        .bg(caps.top)
        .border_1()
        .border_color(theme.border)
        .text_xs()
        .text_color(caps.legend)
        .child(face(action, px(CHIP + 8.0), caps.legend, theme.primary))
}

pub(crate) fn blank_chip(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .size(px(CHIP))
        .rounded(px(TILE_RADIUS))
        .bg(theme::keycaps().top)
        .border_1()
        .border_color(theme.border)
}

pub(crate) fn face(action: Action, height: Pixels, ink: Hsla, accent: Hsla) -> AnyElement {
    match art(action) {
        Some(art) => draw(art, height * ICON_SHARE, ink, accent),
        None => div()
            .px_0p5()
            .text_center()
            .child(SharedString::from(usage::legend(action)))
            .into_any_element(),
    }
}

const fn art(action: Action) -> Option<Art> {
    Some(match action {
        Action::Consumer(code) => match consumer_art(code) {
            Some(art) => art,
            None => return None,
        },
        Action::System(SystemControl::PowerDown) => Art::Icon(Catalog::Power),
        Action::System(SystemControl::Sleep) => Art::Icon(Catalog::Moon),
        Action::System(SystemControl::WakeUp) => Art::Icon(Catalog::Sunrise),
        Action::Function(Function::BrightnessUp) => Art::Signed(Catalog::Lightbulb, "+"),
        Action::Function(Function::BrightnessDown) => Art::Signed(Catalog::Lightbulb, "−"),
        Action::Mouse(button) => Art::Mouse(button),
        _ => return None,
    })
}

const fn consumer_art(code: u16) -> Option<Art> {
    Some(match code {
        0x00B5 => Art::Icon(Catalog::SkipForward),
        0x00B6 => Art::Icon(Catalog::SkipBack),
        0x00B7 => Art::Icon(Catalog::Square),
        0x00CD => Art::Pair(Catalog::Play, Catalog::Pause),
        0x00E2 => Art::Icon(Catalog::VolumeX),
        0x00E9 => Art::Icon(Catalog::Volume2),
        0x00EA => Art::Icon(Catalog::Volume1),
        0x006F => Art::Icon(Catalog::Sun),
        0x0070 => Art::Icon(Catalog::SunDim),
        0x0183 => Art::Icon(Catalog::Music),
        0x018A => Art::Icon(Catalog::Mail),
        0x0192 => Art::Icon(Catalog::Calculator),
        0x0194 => Art::Icon(Catalog::Folder),
        0x0221 => Art::Icon(Catalog::Search),
        0x0223 => Art::Icon(Catalog::Globe),
        0x0224 => Art::Icon(Catalog::ArrowLeft),
        0x0225 => Art::Icon(Catalog::ArrowRight),
        0x0227 => Art::Icon(Catalog::RefreshCw),
        0x022A => Art::Icon(Catalog::Bookmark),
        0x029F => Art::Icon(Catalog::LayoutDashboard),
        0x02A0 => Art::Icon(Catalog::Grid3x3),
        _ => return None,
    })
}

fn draw(art: Art, size: Pixels, ink: Hsla, accent: Hsla) -> AnyElement {
    match art {
        Art::Icon(icon) => Icon::new(icon)
            .size(size)
            .text_color(ink)
            .into_any_element(),
        Art::Signed(icon, sign) => h_flex()
            .items_start()
            .child(Icon::new(icon).size(size).text_color(ink))
            .child(
                div()
                    .text_size(size * 0.7)
                    .line_height(size * 0.7)
                    .text_color(ink)
                    .child(sign),
            )
            .into_any_element(),
        Art::Pair(first, second) => h_flex()
            .child(Icon::new(first).size(size * 0.8).text_color(ink))
            .child(Icon::new(second).size(size * 0.8).text_color(ink))
            .into_any_element(),
        Art::Mouse(button) => mouse(button, size, ink, accent),
    }
}

fn mouse(button: MouseButton, size: Pixels, ink: Hsla, accent: Hsla) -> AnyElement {
    let width = size * 0.8;
    let height = size * 1.2;
    let lit = move |part: MouseButton| {
        if part == button {
            accent
        } else {
            ink.opacity(0.0)
        }
    };
    let thumb = matches!(button, MouseButton::Back | MouseButton::Forward).then(|| {
        div()
            .absolute()
            .left(-px(2.5))
            .top(
                height
                    * if button == MouseButton::Forward {
                        0.48
                    } else {
                        0.68
                    },
            )
            .w(px(3.0))
            .h(height * 0.16)
            .rounded(px(1.0))
            .bg(accent)
    });
    div()
        .relative()
        .w(width)
        .h(height)
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(width / 2.0)
                .border_1()
                .border_color(ink)
                .overflow_hidden()
                .child(
                    h_flex()
                        .w_full()
                        .h(height * 0.42)
                        .border_b_1()
                        .border_color(ink)
                        .child(
                            div()
                                .flex_1()
                                .h_full()
                                .border_r_1()
                                .border_color(ink)
                                .bg(lit(MouseButton::Left)),
                        )
                        .child(div().flex_1().h_full().bg(lit(MouseButton::Right))),
                ),
        )
        .child(
            div()
                .absolute()
                .top(height * 0.1)
                .left(width * 0.5 - px(1.5))
                .w(px(3.0))
                .h(height * 0.2)
                .rounded(px(1.5))
                .bg(if button == MouseButton::Middle {
                    accent
                } else {
                    ink
                }),
        )
        .children(thumb)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::{CONSUMER, MOUSE, SYSTEM};

    #[test]
    fn media_application_system_and_mouse_actions_are_drawn_as_icons() {
        for (code, _, _) in CONSUMER {
            assert!(
                art(Action::Consumer(*code)).is_some(),
                "consumer usage {code:#06x} has no icon"
            );
        }
        for (control, _, _) in SYSTEM {
            assert!(art(Action::System(*control)).is_some());
        }
        for (button, _, _) in MOUSE {
            assert_eq!(art(Action::Mouse(*button)), Some(Art::Mouse(*button)));
        }
    }

    #[test]
    fn characters_keep_their_legend() {
        assert_eq!(art(Action::Keyboard(0x04)), None);
        assert_eq!(
            art(Action::Layer(openwoot_protocol::keys::Layer::One)),
            None
        );
    }
}
