use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, CursorStyle, DispatchPhase, Div, Hitbox, HitboxBehavior, Hsla,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _,
    PathBuilder, Pixels, Point, SharedString, Styled as _, Window, canvas,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, fill, point,
    prelude::FluentBuilder as _,
    px, quad, relative, size,
};
use openwoot_protocol::gamepad::CurvePoint;

use super::curve::{self, Curve, POINT_SCALE};

const HANDLE: f32 = 12.0;
const FOCUSED_HANDLE: f32 = 16.0;
const MARKER: f32 = 8.0;
const GRAB_DISTANCE: f32 = 20.0;
const GRID_LINES: u8 = 3;
const SAMPLES: u16 = 255;
const THUMBNAIL_SAMPLES: u16 = 64;
const HEIGHT: f32 = 220.0;
const AXIS_WIDTH: f32 = 40.0;
const STRIPE_GAP: f32 = 9.0;
const STRIPE_WIDTH: f32 = 3.0;
const TIP_WIDTH: f32 = 140.0;
const TIP_RISE: f32 = 40.0;
const LABEL_MIN_WIDTH: f32 = 0.17;
const LABEL_MIN_HEIGHT: f32 = 0.07;
const THUMBNAIL_INSET: f32 = 6.0;
const THUMBNAIL_DOT: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Input {
    Hover(Option<usize>),
    Grab(usize),
    Move { travel: u8, output: u8 },
    Release,
}

pub(super) type Handler = Rc<dyn Fn(Input, &mut Window, &mut App)>;

pub(super) struct Chart {
    pub(super) curve: Curve,
    pub(super) travel_mm: f32,
    pub(super) focused: Option<usize>,
    pub(super) markers: Vec<(f32, f32)>,
    pub(super) handler: Option<Handler>,
}

#[derive(Clone, Copy)]
struct Colors {
    line: Hsla,
    area: Hsla,
    grid: Hsla,
    stripe: Hsla,
    handle: Hsla,
    marker: Hsla,
}

#[derive(Clone, Copy)]
struct Plot {
    area: Bounds<Pixels>,
}

pub(super) fn chart(chart: Chart, cx: &App) -> Div {
    let theme = cx.theme();
    let colors = Colors {
        line: theme.primary,
        area: theme.primary.opacity(0.85),
        grid: theme.border,
        stripe: theme.secondary,
        handle: theme.background,
        marker: theme.foreground,
    };
    let (muted, border, radius) = (theme.muted_foreground, theme.border, theme.radius);
    let tip_colors = (theme.popover, theme.foreground, theme.border);
    let Chart {
        curve,
        travel_mm,
        focused,
        markers,
        handler,
    } = chart;
    let interactive = handler.is_some();
    let tip = focused
        .and_then(|index| curve.get(index).copied())
        .map(|point| point_tip(point, travel_mm, tip_colors));
    let axis_label = |text: SharedString| div().text_xs().text_color(muted).child(text);
    v_flex()
        .w_full()
        .gap_1()
        .when(!interactive, |this| this.opacity(0.5))
        .child(
            h_flex()
                .pl(px(AXIS_WIDTH))
                .child(axis_label("Analog value".into())),
        )
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .child(
                    v_flex()
                        .w(px(AXIS_WIDTH - 8.0))
                        .h(px(HEIGHT))
                        .items_end()
                        .justify_between()
                        .child(axis_label("255".into()))
                        .child(axis_label("0".into())),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .h(px(HEIGHT))
                        .rounded(radius)
                        .border_1()
                        .border_color(border)
                        .child(drawing(curve, focused, markers, handler, colors))
                        .children(band_labels(curve, muted))
                        .children(tip),
                ),
        )
        .child(
            h_flex()
                .pl(px(AXIS_WIDTH))
                .gap_2()
                .child(axis_label("0.0 mm".into()))
                .child(
                    div()
                        .flex_1()
                        .text_center()
                        .child(axis_label("How deep a key is pressed".into())),
                )
                .child(axis_label(format!("{travel_mm:.1} mm").into())),
        )
}

fn drawing(
    curve: Curve,
    focused: Option<usize>,
    markers: Vec<(f32, f32)>,
    handler: Option<Handler>,
    colors: Colors,
) -> impl IntoElement {
    let interactive = handler.is_some();
    canvas(
        move |bounds, window, _| {
            interactive
                .then(|| window.insert_hitbox(bounds.dilate(px(HANDLE)), HitboxBehavior::Normal))
        },
        move |bounds, hitbox: Option<Hitbox>, window, _| {
            let plot = Plot { area: bounds };
            paint_bands(plot, curve, colors, window);
            paint_grid(plot, colors, window);
            paint_curve(plot, curve, colors, window);
            for (level, output) in &markers {
                window.paint_quad(
                    fill(
                        Bounds::centered_at(
                            plot.position(*level, *output),
                            size(px(MARKER), px(MARKER)),
                        ),
                        colors.marker,
                    )
                    .corner_radii(px(MARKER / 2.0)),
                );
            }
            for (index, point) in curve.iter().enumerate() {
                let chosen = focused == Some(index);
                let diameter = if chosen { FOCUSED_HANDLE } else { HANDLE };
                window.paint_quad(quad(
                    Bounds::centered_at(plot.handle(*point), size(px(diameter), px(diameter))),
                    px(diameter / 2.0),
                    if chosen { colors.line } else { colors.handle },
                    px(2.0),
                    colors.line,
                    BorderStyle::Solid,
                ));
            }
            if let (Some(hitbox), Some(handler)) = (hitbox, handler) {
                listen(plot, curve, &hitbox, &handler, window);
            }
        },
    )
    .absolute()
    .size_full()
}

fn band_labels(curve: Curve, muted: Hsla) -> Vec<Div> {
    let [dead_zone, .., saturation] = curve;
    let start = fraction(dead_zone.travel);
    let end = fraction(saturation.travel);
    let floor = fraction(dead_zone.output);
    let label = |text: &'static str| {
        div()
            .absolute()
            .flex()
            .flex_col()
            .justify_center()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_center()
                    .text_color(muted)
                    .child(text),
            )
    };
    let mut labels = Vec::new();
    if start >= LABEL_MIN_WIDTH {
        labels.push(
            label("Key start deadzone")
                .left_0()
                .top_0()
                .w(relative(start))
                .h_full(),
        );
    }
    if 1.0 - end >= LABEL_MIN_WIDTH {
        labels.push(
            label("Key end deadzone")
                .right_0()
                .top_0()
                .w(relative(1.0 - end))
                .h_full(),
        );
    }
    if floor >= LABEL_MIN_HEIGHT && end > start {
        labels.push(
            label("Anti-deadzone")
                .left(relative(start))
                .bottom_0()
                .w(relative(end - start))
                .h(relative(floor)),
        );
    }
    labels
}

pub(super) fn thumbnail(curve: Curve, line: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let area = Bounds {
                origin: bounds.origin + point(px(THUMBNAIL_INSET), px(THUMBNAIL_INSET)),
                size: size(
                    bounds.size.width - px(2.0 * THUMBNAIL_INSET),
                    bounds.size.height - px(2.0 * THUMBNAIL_INSET),
                ),
            };
            let plot = Plot { area };
            let corners: Vec<Point<Pixels>> =
                curve.iter().map(|point| plot.handle(*point)).collect();
            let mut stroke = PathBuilder::stroke(px(2.0));
            let mut samples = (0..=THUMBNAIL_SAMPLES).map(|step| {
                let level = f32::from(step) / f32::from(THUMBNAIL_SAMPLES);
                plot.position(level, curve::output(curve, level))
            });
            if let Some(first) = samples.next() {
                stroke.move_to(first);
            }
            for sample in samples {
                stroke.line_to(sample);
            }
            if let Ok(path) = stroke.build() {
                window.paint_path(path, line);
            }
            for corner in corners {
                window.paint_quad(
                    fill(
                        Bounds::centered_at(corner, size(px(THUMBNAIL_DOT), px(THUMBNAIL_DOT))),
                        line,
                    )
                    .corner_radii(px(THUMBNAIL_DOT / 2.0)),
                );
            }
        },
    )
    .size_full()
}

pub(super) fn millimetres(travel: u8, travel_mm: f32) -> f32 {
    fraction(travel) * travel_mm
}

fn point_tip(
    point: CurvePoint,
    travel_mm: f32,
    (surface, text, border): (Hsla, Hsla, Hsla),
) -> Div {
    let label = format!(
        "({:.2} mm, {})",
        millimetres(point.travel, travel_mm),
        point.output
    );
    div()
        .absolute()
        .left(relative(fraction(point.travel)))
        .top(relative(1.0 - fraction(point.output)))
        .ml(px(-TIP_WIDTH / 2.0))
        .mt(px(-TIP_RISE))
        .w(px(TIP_WIDTH))
        .flex()
        .justify_center()
        .child(
            div()
                .px_2()
                .py_1()
                .rounded(px(6.0))
                .bg(surface)
                .border_1()
                .border_color(border)
                .shadow_md()
                .text_xs()
                .text_color(text)
                .child(label),
        )
}

fn fraction(value: u8) -> f32 {
    f32::from(value) / POINT_SCALE
}

impl Plot {
    fn position(self, level: f32, output: f32) -> Point<Pixels> {
        point(
            self.area.origin.x + self.area.size.width * level.clamp(0.0, 1.0),
            self.area.origin.y + self.area.size.height * (1.0 - output.clamp(0.0, 1.0)),
        )
    }

    fn handle(self, point: CurvePoint) -> Point<Pixels> {
        self.position(fraction(point.travel), fraction(point.output))
    }

    fn values(self, position: Point<Pixels>) -> (u8, u8) {
        let width = f32::from(self.area.size.width).max(1.0);
        let height = f32::from(self.area.size.height).max(1.0);
        let level = f32::from(position.x - self.area.origin.x) / width;
        let output = 1.0 - f32::from(position.y - self.area.origin.y) / height;
        (
            curve::point_value(level * POINT_SCALE),
            curve::point_value(output * POINT_SCALE),
        )
    }

    fn rectangle(self, (left, right): (f32, f32), (bottom, top): (f32, f32)) -> Bounds<Pixels> {
        Bounds::from_corners(self.position(left, top), self.position(right, bottom))
    }
}

fn paint_bands(plot: Plot, curve: Curve, colors: Colors, window: &mut Window) {
    let [dead_zone, .., saturation] = curve;
    let start = fraction(dead_zone.travel);
    let end = fraction(saturation.travel);
    let floor = fraction(dead_zone.output);
    let bands = [
        ((0.0, start), (0.0, 1.0)),
        ((end, 1.0), (0.0, 1.0)),
        ((start, end), (0.0, floor)),
    ];
    for (across, up) in bands {
        if across.1 > across.0 && up.1 > up.0 {
            hatch(plot.rectangle(across, up), colors.stripe, window);
        }
    }
}

fn hatch(band: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let left = f32::from(band.origin.x);
    let top = f32::from(band.origin.y);
    let right = left + f32::from(band.size.width);
    let bottom = top + f32::from(band.size.height);
    let mut offset = left - bottom;
    while offset < right - top {
        let from_x = left.max(top + offset);
        let to_x = right.min(bottom + offset);
        if from_x < to_x {
            let mut stripe = PathBuilder::stroke(px(STRIPE_WIDTH));
            stripe.move_to(point(px(from_x), px(from_x - offset)));
            stripe.line_to(point(px(to_x), px(to_x - offset)));
            if let Ok(path) = stripe.build() {
                window.paint_path(path, color);
            }
        }
        offset += STRIPE_GAP;
    }
}

fn paint_grid(plot: Plot, colors: Colors, window: &mut Window) {
    for line in 1..GRID_LINES {
        let fraction = f32::from(line) / f32::from(GRID_LINES);
        window.paint_quad(fill(
            Bounds {
                origin: plot.position(fraction, 1.0),
                size: size(px(1.0), plot.area.size.height),
            },
            colors.grid,
        ));
        window.paint_quad(fill(
            Bounds {
                origin: plot.position(0.0, fraction),
                size: size(plot.area.size.width, px(1.0)),
            },
            colors.grid,
        ));
    }
}

fn paint_curve(plot: Plot, curve: Curve, colors: Colors, window: &mut Window) {
    let [dead_zone, .., saturation] = curve;
    let start = fraction(dead_zone.travel);
    let end = fraction(saturation.travel);
    let floor = fraction(dead_zone.output);
    let samples: Vec<(f32, Point<Pixels>)> = (0..=SAMPLES)
        .map(|step| {
            let level = f32::from(step) / f32::from(SAMPLES);
            (level, plot.position(level, curve::output(curve, level)))
        })
        .collect();
    if end > start {
        let mut area = PathBuilder::fill();
        area.move_to(plot.position(start, floor));
        for (_, sample) in samples
            .iter()
            .filter(|(level, _)| *level > start && *level <= end)
        {
            area.line_to(*sample);
        }
        area.line_to(plot.position(end, curve::output(curve, end)));
        area.line_to(plot.position(end, floor));
        area.close();
        if let Ok(path) = area.build() {
            window.paint_path(path, colors.area);
        }
    }
    let mut line = PathBuilder::stroke(px(2.0));
    let mut points = samples.iter().map(|(_, sample)| *sample);
    if let Some(first) = points.next() {
        line.move_to(first);
    }
    for sample in points {
        line.line_to(sample);
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, colors.line);
    }
}

fn listen(plot: Plot, curve: Curve, hitbox: &Hitbox, handler: &Handler, window: &mut Window) {
    window.set_cursor_style(CursorStyle::PointingHand, hitbox);
    let grips: Rc<Vec<Point<Pixels>>> =
        Rc::new(curve.iter().map(|point| plot.handle(*point)).collect());
    let pressed_on = hitbox.clone();
    let grab = handler.clone();
    let grab_grips = grips.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !pressed_on.is_hovered(window)
        {
            return;
        }
        if let Some(index) = nearest(&grab_grips, event.position) {
            grab(Input::Grab(index), window, cx);
            cx.stop_propagation();
        }
    });
    let hovered_on = hitbox.clone();
    let moved = handler.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble {
            return;
        }
        if event.pressed_button == Some(MouseButton::Left) {
            let (travel, output) = plot.values(event.position);
            moved(Input::Move { travel, output }, window, cx);
        } else {
            let hovered = hovered_on
                .is_hovered(window)
                .then(|| nearest(&grips, event.position))
                .flatten();
            moved(Input::Hover(hovered), window, cx);
        }
    });
    let release = handler.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
            release(Input::Release, window, cx);
        }
    });
}

fn nearest(grips: &[Point<Pixels>], position: Point<Pixels>) -> Option<usize> {
    grips
        .iter()
        .enumerate()
        .map(|(index, grip)| (index, distance(*grip, position)))
        .filter(|(_, distance)| *distance <= GRAB_DISTANCE)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(index, _)| index)
}

fn distance(from: Point<Pixels>, to: Point<Pixels>) -> f32 {
    let across = f32::from(from.x - to.x);
    let down = f32::from(from.y - to.y);
    across.hypot(down)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_grip_within_reach_is_picked() {
        let grips = [point(px(0.0), px(0.0)), point(px(30.0), px(0.0))];
        assert_eq!(nearest(&grips, point(px(18.0), px(0.0))), Some(1));
        assert_eq!(nearest(&grips, point(px(4.0), px(3.0))), Some(0));
        assert_eq!(nearest(&grips, point(px(15.0), px(40.0))), None);
    }

    #[test]
    fn curve_points_read_as_millimetres_of_the_switch() {
        assert!((millimetres(255, 4.0) - 4.0).abs() < 1e-6);
        assert!((millimetres(60, 4.0) - 0.941).abs() < 1e-3);
    }
}
