use std::rc::Rc;

use gpui_kit::{
    App, Bounds, BoxShadow, ContentMask, CursorStyle, DispatchPhase, Div, FontWeight, Hitbox,
    HitboxBehavior, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, PathBuilder, Pixels, Point, SharedString, Styled as _, Window, canvas,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, fill, hsla, point,
    prelude::FluentBuilder as _,
    px, relative, size,
};
use openwoot_protocol::gamepad::CurvePoint;

use super::curve::{self, Curve, POINT_SCALE};
use crate::theme;

const HANDLE: f32 = 12.0;
const MARKER: f32 = 8.0;
const GRAB_DISTANCE: f32 = 20.0;
const GRID_STEP_ACROSS: f32 = 96.0;
const GRID_STEP_DOWN: f32 = 60.0;
const SAMPLES: u16 = 255;
const THUMBNAIL_SAMPLES: u16 = 64;
const MAX_WIDTH: f32 = 332.0;
const ASPECT_RATIO: f32 = 1.6;
const RANGE_SIZE: f32 = 48.0;
const METRIC_WIDTH: f32 = 64.0;
const METRIC_ROW: f32 = 32.0;
const METRIC_TOP: f32 = 12.0;
const LABEL_TEXT: f32 = 12.0;
const LABEL_LINE: f32 = 18.0;
const BAND_LABEL_TEXT: f32 = 16.0;
const BAND_LABEL_LINE: f32 = 24.0;
const LINE_WIDTH: f32 = 2.5;
const STRIPE: f32 = 10.0;
const TIP_WIDTH: f32 = 200.0;
const TIP_GAP: f32 = 14.0;
const TIP_PADDING_ACROSS: f32 = 12.0;
const TIP_PADDING_DOWN: f32 = 8.0;
const TIP_RADIUS: f32 = 6.0;
const TIP_TEXT: f32 = 14.0;
const TIP_LINE: f32 = 21.0;
const DISABLED: f32 = 0.5;
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
    stripe_gap: Hsla,
    marker: Hsla,
}

#[derive(Clone, Copy)]
struct Plot {
    area: Bounds<Pixels>,
}

pub(super) fn chart(chart: Chart, cx: &App) -> Div {
    let theme = cx.theme();
    let colors = Colors {
        line: theme.chart_2,
        area: theme.chart_1,
        grid: theme.chart_3,
        stripe: theme.chart_4,
        stripe_gap: theme.chart_5,
        marker: theme.foreground,
    };
    let axis = theme.chart_grid;
    let tip_colors = (theme::palette().tooltip, theme.foreground, theme.border);
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
    let plot = div()
        .relative()
        .flex_1()
        .min_h_0()
        .w_full()
        .border_1()
        .border_color(axis)
        .child(underlay(curve, colors))
        .children(band_labels(curve, axis))
        .child(overlay(curve, markers, handler, colors))
        .children(tip);
    h_flex()
        .w_full()
        .max_w(px(MAX_WIDTH))
        .aspect_ratio(ASPECT_RATIO)
        .items_start()
        .when(!interactive, |this| this.opacity(DISABLED))
        .child(ranges(axis))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(plot)
                .child(metrics(travel_mm, axis)),
        )
}

fn axis_label(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(px(LABEL_TEXT))
        .line_height(px(LABEL_LINE))
        .text_color(color)
        .child(text.into())
}

fn ranges(color: Hsla) -> Div {
    v_flex()
        .flex_none()
        .w(px(RANGE_SIZE))
        .h_full()
        .child(
            h_flex()
                .flex_none()
                .h(px(RANGE_SIZE))
                .items_start()
                .justify_center()
                .child(axis_label("255", color)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .justify_center()
                .text_center()
                .child(axis_label("Analog Value", color).w_full()),
        )
        .child(
            h_flex()
                .flex_none()
                .h(px(RANGE_SIZE))
                .items_end()
                .justify_center()
                .child(axis_label("0", color)),
        )
        .child(div().flex_none().h(px(METRIC_ROW)))
}

fn metrics(travel_mm: f32, color: Hsla) -> Div {
    h_flex()
        .flex_none()
        .w_full()
        .h(px(METRIC_ROW))
        .pt(px(METRIC_TOP))
        .items_start()
        .child(
            div()
                .flex_none()
                .w(px(METRIC_WIDTH))
                .child(axis_label("0.00 mm", color)),
        )
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .justify_center()
                .child(axis_label("How Deep a Key is Pressed", color)),
        )
        .child(
            h_flex()
                .flex_none()
                .w(px(METRIC_WIDTH))
                .justify_end()
                .child(axis_label(format!("{travel_mm:.2} mm"), color)),
        )
}

fn underlay(curve: Curve, colors: Colors) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let plot = Plot { area: bounds };
            paint_grid(plot, colors, window);
            paint_curve(plot, curve, colors, window);
            paint_bands(plot, curve, colors, window);
        },
    )
    .absolute()
    .size_full()
}

fn overlay(
    curve: Curve,
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
            for point in curve {
                window.paint_quad(
                    fill(
                        Bounds::centered_at(plot.handle(point), size(px(HANDLE), px(HANDLE))),
                        colors.line,
                    )
                    .corner_radii(px(HANDLE / 2.0)),
                );
            }
            if let (Some(hitbox), Some(handler)) = (hitbox, handler) {
                listen(plot, curve, &hitbox, &handler, window);
            }
        },
    )
    .absolute()
    .size_full()
}

fn band_labels(curve: Curve, color: Hsla) -> Vec<Div> {
    let [dead_zone, .., saturation] = curve;
    let start = fraction(dead_zone.travel);
    let end = fraction(saturation.travel);
    let floor = fraction(dead_zone.output);
    let label = |text: &'static str| {
        div().absolute().flex().flex_col().justify_center().child(
            div()
                .w_full()
                .text_size(px(BAND_LABEL_TEXT))
                .line_height(px(BAND_LABEL_LINE))
                .text_center()
                .text_color(color)
                .child(text),
        )
    };
    let mut labels = Vec::new();
    if start >= LABEL_MIN_WIDTH {
        labels.push(
            label("Key Start Deadzone")
                .left_0()
                .top_0()
                .w(relative(start))
                .h_full(),
        );
    }
    if 1.0 - end >= LABEL_MIN_WIDTH {
        labels.push(
            label("Key End Deadzone")
                .right_0()
                .top_0()
                .w(relative(1.0 - end))
                .h_full(),
        );
    }
    if floor >= LABEL_MIN_HEIGHT && end > start {
        labels.push(
            label("Anti-Deadzone")
                .justify_start()
                .overflow_hidden()
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
        .bottom(relative(fraction(point.output)))
        .ml(px(-TIP_WIDTH / 2.0))
        .mb(px(TIP_GAP))
        .w(px(TIP_WIDTH))
        .flex()
        .justify_center()
        .child(
            div()
                .px(px(TIP_PADDING_ACROSS))
                .py(px(TIP_PADDING_DOWN))
                .rounded(px(TIP_RADIUS))
                .bg(surface)
                .border_1()
                .border_color(border)
                .shadow(tip_shadow())
                .text_size(px(TIP_TEXT))
                .line_height(px(TIP_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text)
                .child(label),
        )
}

fn tip_shadow() -> Vec<BoxShadow> {
    [(0.1, 4.0, 6.0), (0.06, 2.0, 4.0)]
        .into_iter()
        .map(|(alpha, rise, blur)| BoxShadow {
            color: hsla(0.0, 0.0, 0.0, alpha),
            offset: point(px(0.0), px(rise)),
            blur_radius: px(blur),
            spread_radius: px(-1.0),
            inset: false,
        })
        .collect()
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
            hatch(plot.rectangle(across, up), colors, window);
        }
    }
}

fn hatch(band: Bounds<Pixels>, colors: Colors, window: &mut Window) {
    window.paint_quad(fill(band, colors.stripe_gap));
    let left = f32::from(band.origin.x);
    let top = f32::from(band.origin.y);
    let right = left + f32::from(band.size.width);
    let bottom = top + f32::from(band.size.height);
    let step = 2.0 * STRIPE * std::f32::consts::SQRT_2;
    window.with_content_mask(Some(ContentMask { bounds: band }), |window| {
        let mut offset = left - bottom - step;
        while offset < right - top + step {
            let mut stripe = PathBuilder::stroke(px(STRIPE));
            stripe.move_to(point(px(top + offset - STRIPE), px(top - STRIPE)));
            stripe.line_to(point(px(bottom + offset + STRIPE), px(bottom + STRIPE)));
            if let Ok(path) = stripe.build() {
                window.paint_path(path, colors.stripe);
            }
            offset += step;
        }
    });
}

fn paint_grid(plot: Plot, colors: Colors, window: &mut Window) {
    let area = plot.area;
    let mut across = 0.0;
    while across < f32::from(area.size.width) {
        window.paint_quad(fill(
            Bounds {
                origin: area.origin + point(px(across), px(0.0)),
                size: size(px(1.0), area.size.height),
            },
            colors.grid,
        ));
        across += GRID_STEP_ACROSS;
    }
    let mut down = 0.0;
    while down < f32::from(area.size.height) {
        window.paint_quad(fill(
            Bounds {
                origin: area.origin + point(px(0.0), px(down)),
                size: size(area.size.width, px(1.0)),
            },
            colors.grid,
        ));
        down += GRID_STEP_DOWN;
    }
}

fn paint_curve(plot: Plot, curve: Curve, colors: Colors, window: &mut Window) {
    let samples: Vec<Point<Pixels>> = (0..=SAMPLES)
        .map(|step| {
            let level = f32::from(step) / f32::from(SAMPLES);
            plot.position(level, curve::output(curve, level))
        })
        .collect();
    let mut area = PathBuilder::fill();
    area.move_to(plot.position(0.0, 0.0));
    for sample in &samples {
        area.line_to(*sample);
    }
    area.line_to(plot.position(1.0, 0.0));
    area.close();
    if let Ok(path) = area.build() {
        window.paint_path(path, colors.area);
    }
    let mut line = PathBuilder::stroke(px(LINE_WIDTH));
    let mut points = samples.iter().copied();
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
