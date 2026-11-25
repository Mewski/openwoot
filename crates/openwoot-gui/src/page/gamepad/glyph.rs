use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use gpui_kit::{
    App, Bounds, Div, FontWeight, Hsla, IntoElement, ParentElement as _, PathBuilder, Pixels,
    Point, Styled as _, Window, canvas, component::ActiveTheme as _, div, point,
    prelude::FluentBuilder as _, px, relative, rgb,
};
use openwoot_protocol::gamepad::Control;

use super::controls::{self, Controller};
use crate::keyboard::Glyph;

const KEY_SCALE: f32 = 0.84;
const FADED: f32 = 0.35;

#[derive(Debug, Clone, Copy)]
pub(super) struct Ink {
    body: Hsla,
    rim: Hsla,
    mark: Hsla,
    accent: Hsla,
    shade: Hsla,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paint {
    Green,
    Red,
    Blue,
    Amber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Face(&'static str, Paint),
    Menu(Direction),
    Guide,
    Pad(Direction),
    Stick(&'static str, Option<Direction>),
    Rotation(&'static str, bool),
    Bumper(&'static str, Direction),
    Trigger(&'static str, Direction),
    Shield(&'static str, Paint),
    Number(&'static str),
}

impl Ink {
    pub(super) fn of(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            body: theme.muted_foreground,
            rim: theme.sidebar_foreground,
            mark: theme.foreground,
            accent: theme.primary,
            shade: theme.switch,
        }
    }
}

pub(super) fn glyph(control: Control, controller: Controller, ink: Ink, faded: bool) -> Glyph {
    Glyph::new(move |height| {
        icon(control, controller, ink, height * KEY_SCALE)
            .when(faded, |this| this.opacity(FADED))
            .into_any_element()
    })
}

pub(super) fn icon(control: Control, controller: Controller, ink: Ink, size: Pixels) -> Div {
    let shape = match controls::shown_as(control, controller) {
        Controller::Xbox => xbox_shape(control),
        Controller::Classic => classic_shape(control),
    };
    draw(shape, ink, f32::from(size))
}

const fn xbox_shape(control: Control) -> Shape {
    match control {
        Control::ButtonA => Shape::Face("A", Paint::Green),
        Control::ButtonB => Shape::Face("B", Paint::Red),
        Control::ButtonX => Shape::Face("X", Paint::Blue),
        Control::ButtonY => Shape::Face("Y", Paint::Amber),
        Control::Start => Shape::Menu(Direction::Right),
        Control::Back => Shape::Menu(Direction::Left),
        Control::Guide => Shape::Guide,
        Control::DpadUp => Shape::Pad(Direction::Up),
        Control::DpadDown => Shape::Pad(Direction::Down),
        Control::DpadLeft => Shape::Pad(Direction::Left),
        Control::DpadRight => Shape::Pad(Direction::Right),
        Control::LeftStickUp => Shape::Stick("L", Some(Direction::Up)),
        Control::LeftStickDown => Shape::Stick("L", Some(Direction::Down)),
        Control::LeftStickLeft => Shape::Stick("L", Some(Direction::Left)),
        Control::LeftStickRight => Shape::Stick("L", Some(Direction::Right)),
        Control::LeftStickPress => Shape::Stick("L", None),
        Control::RightStickUp => Shape::Stick("R", Some(Direction::Up)),
        Control::RightStickDown => Shape::Stick("R", Some(Direction::Down)),
        Control::RightStickLeft => Shape::Stick("R", Some(Direction::Left)),
        Control::RightStickRight => Shape::Stick("R", Some(Direction::Right)),
        Control::RightStickPress => Shape::Stick("R", None),
        Control::LeftBumper => Shape::Bumper("LB", Direction::Left),
        Control::RightBumper => Shape::Bumper("RB", Direction::Right),
        Control::LeftTrigger => Shape::Trigger("LT", Direction::Left),
        Control::RightTrigger => Shape::Trigger("RT", Direction::Right),
        Control::Accelerator
        | Control::Rudder
        | Control::Trigger
        | Control::RxNegative
        | Control::RxPositive
        | Control::RyNegative
        | Control::RyPositive
        | Control::HatUp
        | Control::HatDown
        | Control::HatLeft
        | Control::HatRight => classic_shape(control),
    }
}

const fn classic_shape(control: Control) -> Shape {
    match control {
        Control::LeftStickLeft => Shape::Stick("X", Some(Direction::Left)),
        Control::LeftStickRight => Shape::Stick("X", Some(Direction::Right)),
        Control::LeftStickUp => Shape::Stick("Y", Some(Direction::Up)),
        Control::LeftStickDown => Shape::Stick("Y", Some(Direction::Down)),
        Control::RxNegative => Shape::Stick("Z", Some(Direction::Up)),
        Control::RxPositive => Shape::Stick("Z", Some(Direction::Down)),
        Control::RyNegative => Shape::Rotation("Z", false),
        Control::RyPositive => Shape::Rotation("Z", true),
        Control::RightStickLeft => Shape::Rotation("X", false),
        Control::RightStickRight => Shape::Rotation("X", true),
        Control::RightStickUp => Shape::Rotation("Y", false),
        Control::RightStickDown => Shape::Rotation("Y", true),
        Control::LeftTrigger => Shape::Trigger("TH", Direction::Left),
        Control::RightTrigger => Shape::Shield("E", Paint::Amber),
        Control::Accelerator => Shape::Shield("A", Paint::Green),
        Control::Rudder => Shape::Shield("R", Paint::Blue),
        Control::Trigger => Shape::Shield("T", Paint::Red),
        Control::ButtonA => Shape::Number("1"),
        Control::ButtonB => Shape::Number("2"),
        Control::ButtonX => Shape::Number("3"),
        Control::ButtonY => Shape::Number("4"),
        Control::Start => Shape::Number("5"),
        Control::Back => Shape::Number("6"),
        Control::DpadUp => Shape::Number("7"),
        Control::DpadDown => Shape::Number("8"),
        Control::DpadLeft => Shape::Number("9"),
        Control::DpadRight => Shape::Number("10"),
        Control::LeftBumper => Shape::Number("11"),
        Control::RightBumper => Shape::Number("12"),
        Control::HatUp => Shape::Pad(Direction::Up),
        Control::HatDown => Shape::Pad(Direction::Down),
        Control::HatLeft => Shape::Pad(Direction::Left),
        Control::HatRight => Shape::Pad(Direction::Right),
        Control::Guide | Control::LeftStickPress | Control::RightStickPress => xbox_shape(control),
    }
}

impl Paint {
    fn colors(self) -> (Hsla, Hsla) {
        let (fill, ring) = match self {
            Self::Green => (0x0022_c55e, 0x0015_803d),
            Self::Red => (0x00ef_4444, 0x00b9_1c1c),
            Self::Blue => (0x003b_82f6, 0x001d_4ed8),
            Self::Amber => (0x00ea_b308, 0x00a1_6207),
        };
        (rgb(fill).into(), rgb(ring).into())
    }
}

impl Direction {
    const fn angle(self) -> f32 {
        match self {
            Self::Up => -FRAC_PI_2,
            Self::Down => FRAC_PI_2,
            Self::Left => PI,
            Self::Right => 0.0,
        }
    }
}

fn draw(shape: Shape, ink: Ink, size: f32) -> Div {
    let frame = div()
        .size(px(size))
        .flex_none()
        .relative()
        .flex()
        .items_center()
        .justify_center();
    match shape {
        Shape::Face(letter, paint) => frame.child(coloured_disc(letter, paint, ink, size)),
        Shape::Menu(direction) => frame.child(menu(direction, ink, size)),
        Shape::Guide => frame.child(guide(ink, size)),
        Shape::Pad(direction) => pad(frame, direction, ink, size),
        Shape::Stick(letter, direction) => stick(frame, letter, direction, ink, size),
        Shape::Rotation(letter, clockwise) => rotation(frame, letter, clockwise, ink, size),
        Shape::Bumper(label, hand) => frame.child(shoulder(label, hand, false, ink, size)),
        Shape::Trigger(label, hand) => frame.child(shoulder(label, hand, true, ink, size)),
        Shape::Shield(letter, paint) => frame.child(shield(letter, paint, ink, size)),
        Shape::Number(label) => frame.child(numbered(label, ink, size)),
    }
}

fn coloured_disc(letter: &'static str, paint: Paint, ink: Ink, size: f32) -> Div {
    let (fill, ring) = paint.colors();
    disc(size * 0.92)
        .bg(fill)
        .border(px(size * 0.06))
        .border_color(ring)
        .child(letter_of(letter, size * 0.42, ink.mark))
}

fn menu(direction: Direction, ink: Ink, size: f32) -> Div {
    div()
        .relative()
        .w(px(size * 0.86))
        .h(px(size * 0.5))
        .rounded(px(size * 0.12))
        .bg(ink.mark)
        .border(px(size * 0.03))
        .border_color(ink.rim)
        .child(art(move |bounds, window| {
            let centre = bounds.center();
            let tip = if direction == Direction::Left {
                -1.0
            } else {
                1.0
            };
            let mut outline = PathBuilder::stroke(px(size * 0.05));
            outline.move_to(offset(centre, -0.1 * size * tip, -0.13 * size));
            outline.line_to(offset(centre, 0.14 * size * tip, 0.0));
            outline.line_to(offset(centre, -0.1 * size * tip, 0.13 * size));
            outline.close();
            paint(outline, ink.body, window);
        }))
}

fn guide(ink: Ink, size: f32) -> Div {
    disc(size * 0.9)
        .bg(ink.body)
        .border(px(size * 0.07))
        .border_color(ink.accent)
        .child(art(move |bounds, window| {
            let centre = bounds.center();
            let reach = size * 0.3;
            for angle in [FRAC_PI_4, 3.0 * FRAC_PI_4] {
                let mut stroke = PathBuilder::stroke(px(size * 0.07));
                stroke.move_to(polar(centre, reach, angle));
                stroke.line_to(polar(centre, reach, angle + PI));
                paint(stroke, ink.accent, window);
            }
        }))
}

fn pad(frame: Div, direction: Direction, ink: Ink, size: f32) -> Div {
    frame
        .child(centred_bar(size, size * 0.32, size * 0.92, ink.body))
        .child(centred_bar(size, size * 0.92, size * 0.32, ink.body))
        .child(art(move |bounds, window| {
            let centre = bounds.center();
            let angle = direction.angle();
            let base = polar(centre, size * 0.27, angle);
            let across = size * 0.1;
            triangle(
                [
                    polar(centre, size * 0.41, angle),
                    polar(base, across, angle + FRAC_PI_2),
                    polar(base, across, angle - FRAC_PI_2),
                ],
                ink.mark,
                window,
            );
        }))
}

fn stick(
    frame: Div,
    letter: &'static str,
    direction: Option<Direction>,
    ink: Ink,
    size: f32,
) -> Div {
    let pressed = direction.is_none();
    frame
        .child(
            disc(size * 0.94)
                .absolute()
                .left(px(size * 0.03))
                .top(px(size * 0.03))
                .border(px(size * if pressed { 0.07 } else { 0.035 }))
                .border_color(if pressed { ink.accent } else { ink.rim }),
        )
        .child(
            disc(size * 0.64)
                .bg(ink.body)
                .child(letter_of(letter, size * 0.34, ink.mark)),
        )
        .when_some(direction, |this, direction| {
            this.child(art(move |bounds, window| {
                let angle = direction.angle();
                arc(
                    bounds.center(),
                    size * 0.43,
                    (angle - FRAC_PI_4, angle + FRAC_PI_4),
                    size * 0.08,
                    ink.accent,
                    window,
                );
            }))
        })
}

fn rotation(frame: Div, letter: &'static str, clockwise: bool, ink: Ink, size: f32) -> Div {
    frame
        .child(
            disc(size * 0.58)
                .bg(ink.body)
                .child(letter_of(letter, size * 0.32, ink.mark)),
        )
        .child(art(move |bounds, window| {
            let centre = bounds.center();
            let radius = size * 0.38;
            let (start, end) = if clockwise {
                (-FRAC_PI_2 + 0.5, -FRAC_PI_2 + 2.0 * PI - 0.9)
            } else {
                (-FRAC_PI_2 - 0.5, -FRAC_PI_2 - 2.0 * PI + 0.9)
            };
            arc(centre, radius, (start, end), size * 0.08, ink.mark, window);
            let head = polar(centre, radius, end);
            let forward = if clockwise {
                end + FRAC_PI_2
            } else {
                end - FRAC_PI_2
            };
            triangle(
                [
                    polar(head, size * 0.13, forward),
                    polar(head, size * 0.1, end),
                    polar(head, size * 0.1, end + PI),
                ],
                ink.mark,
                window,
            );
        }))
}

fn shoulder(label: &'static str, hand: Direction, trigger: bool, ink: Ink, size: f32) -> Div {
    let (width, height, curve) = if trigger {
        (0.76, 0.84, 0.38)
    } else {
        (0.88, 0.56, 0.3)
    };
    let curve = px(size * curve);
    div()
        .w(px(size * width))
        .h(px(size * height))
        .flex()
        .items_center()
        .justify_center()
        .bg(ink.body)
        .rounded(px(size * 0.08))
        .map(|this| match (trigger, hand) {
            (false, Direction::Left) => this.rounded_tl(curve),
            (false, _) => this.rounded_tr(curve),
            (true, Direction::Left) => this.rounded_bl(curve),
            (true, _) => this.rounded_br(curve),
        })
        .child(letter_of(label, size * 0.3, ink.mark))
}

fn shield(letter: &'static str, paint: Paint, ink: Ink, size: f32) -> Div {
    let (fill, ring) = paint.colors();
    div()
        .w(px(size * 0.72))
        .h(px(size * 0.84))
        .flex()
        .items_center()
        .justify_center()
        .bg(fill)
        .border(px(size * 0.05))
        .border_color(ring)
        .rounded_t(px(size * 0.08))
        .rounded_b(px(size * 0.36))
        .child(letter_of(letter, size * 0.36, ink.mark))
}

fn numbered(label: &'static str, ink: Ink, size: f32) -> Div {
    let text = if label.len() > 1 { 0.32 } else { 0.4 };
    disc(size * 0.88)
        .bg(ink.body)
        .border(px(size * 0.07))
        .border_color(ink.shade)
        .child(letter_of(label, size * text, ink.mark))
}

fn disc(diameter: f32) -> Div {
    div()
        .size(px(diameter))
        .flex_none()
        .relative()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
}

fn letter_of(text: &'static str, size: f32, color: Hsla) -> Div {
    div()
        .text_size(px(size))
        .line_height(relative(1.0))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(text)
}

fn centred_bar(size: f32, width: f32, height: f32, color: Hsla) -> Div {
    div()
        .absolute()
        .left(px((size - width) / 2.0))
        .top(px((size - height) / 2.0))
        .w(px(width))
        .h(px(height))
        .rounded(px(size * 0.06))
        .bg(color)
}

fn art(paint: impl FnOnce(Bounds<Pixels>, &mut Window) + 'static) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| paint(bounds, window),
    )
    .absolute()
    .size_full()
}

fn arc(
    centre: Point<Pixels>,
    radius: f32,
    (from, to): (f32, f32),
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    let mut stroke = PathBuilder::stroke(px(width));
    stroke.move_to(polar(centre, radius, from));
    stroke.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        (to - from).abs() > PI,
        to > from,
        polar(centre, radius, to),
    );
    paint(stroke, color, window);
}

fn triangle([first, second, third]: [Point<Pixels>; 3], color: Hsla, window: &mut Window) {
    let mut fill = PathBuilder::fill();
    fill.move_to(first);
    fill.line_to(second);
    fill.line_to(third);
    fill.close();
    paint(fill, color, window);
}

fn paint(path: PathBuilder, color: Hsla, window: &mut Window) {
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn polar(centre: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    offset(centre, radius * angle.cos(), radius * angle.sin())
}

fn offset(origin: Point<Pixels>, across: f32, down: f32) -> Point<Pixels> {
    point(origin.x + px(across), origin.y + px(down))
}

#[cfg(test)]
mod tests {
    use super::{
        controls::{CLASSIC, XBOX},
        *,
    };

    #[test]
    fn every_palette_entry_draws_its_own_shape() {
        for palette in [XBOX.as_slice(), CLASSIC.as_slice()] {
            let controller = if palette.len() == XBOX.len() {
                Controller::Xbox
            } else {
                Controller::Classic
            };
            let shapes: Vec<Shape> = palette
                .iter()
                .map(|control| match controls::shown_as(*control, controller) {
                    Controller::Xbox => xbox_shape(*control),
                    Controller::Classic => classic_shape(*control),
                })
                .collect();
            for (index, shape) in shapes.iter().enumerate() {
                assert!(!shapes.iter().skip(index + 1).any(|other| other == shape));
            }
        }
    }

    #[test]
    fn classic_draws_numbered_buttons_and_xbox_draws_coloured_faces() {
        assert_eq!(classic_shape(Control::ButtonX), Shape::Number("3"));
        assert_eq!(xbox_shape(Control::ButtonX), Shape::Face("X", Paint::Blue));
        assert_eq!(classic_shape(Control::Guide), Shape::Guide);
        assert_eq!(xbox_shape(Control::HatUp), Shape::Pad(Direction::Up));
    }
}
