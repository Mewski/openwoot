use std::rc::Rc;

use gpui_kit::{
    App, BorderStyle, Bounds, CursorStyle, DispatchPhase, Div, Hitbox, HitboxBehavior, Hsla,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, PathBuilder,
    Pixels, Point, Styled as _, Window, canvas,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, fill, point, px, quad, size,
};
use openwoot_protocol::gamepad::CurvePoint;

use super::curve::{self, Curve, POINT_SCALE};

const INSET: f32 = 10.0;
const HANDLE: f32 = 12.0;
const MARKER: f32 = 8.0;
const GRAB_DISTANCE: f32 = 24.0;
const GRID_LINES: u8 = 4;
const SAMPLES: u16 = 255;
const HEIGHT: f32 = 240.0;
const MAX_WIDTH: f32 = 520.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Input {
    Grab(usize),
    Move { travel: u8, output: u8 },
    Release,
}

pub(super) type Handler = Rc<dyn Fn(Input, &mut Window, &mut App)>;

pub(super) struct Chart {
    pub(super) curve: Curve,
    pub(super) selected: Option<usize>,
    pub(super) markers: Vec<(f32, f32)>,
    pub(super) handler: Option<Handler>,
}

#[derive(Clone, Copy)]
struct Colors {
    line: Hsla,
    area: Hsla,
    grid: Hsla,
    handle: Hsla,
    selected: Hsla,
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
        area: theme.primary.opacity(0.12),
        grid: theme.border,
        handle: theme.background,
        selected: theme.primary,
        marker: theme.warning,
    };
    let muted = theme.muted_foreground;
    let radius = theme.radius;
    let border = theme.border;
    let Chart {
        curve,
        selected,
        markers,
        handler,
    } = chart;
    let interactive = handler.is_some();
    let plot = canvas(
        move |bounds, window, _| {
            interactive.then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
        },
        move |bounds, hitbox: Option<Hitbox>, window, _| {
            let plot = Plot::new(bounds);
            paint_grid(plot, colors, window);
            paint_curve(plot, curve, colors, window);
            for (level, output) in &markers {
                let center = plot.position(*level, *output);
                window.paint_quad(
                    fill(
                        Bounds::centered_at(center, size(px(MARKER), px(MARKER))),
                        colors.marker,
                    )
                    .corner_radii(px(MARKER / 2.0)),
                );
            }
            for (index, point) in curve.iter().enumerate() {
                let chosen = selected == Some(index);
                window.paint_quad(quad(
                    Bounds::centered_at(plot.handle(*point), size(px(HANDLE), px(HANDLE))),
                    px(HANDLE / 2.0),
                    if chosen {
                        colors.selected
                    } else {
                        colors.handle
                    },
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
    .size_full();
    let axis_label = |text: &'static str| div().text_xs().text_color(muted).child(text);
    v_flex()
        .w_full()
        .max_w(px(MAX_WIDTH))
        .gap_1()
        .child(
            h_flex()
                .gap_2()
                .child(axis_label("Analog value"))
                .child(div().flex_1())
                .child(axis_label("100%")),
        )
        .child(
            div()
                .w_full()
                .h(px(HEIGHT))
                .rounded(radius)
                .border_1()
                .border_color(border)
                .child(plot),
        )
        .child(
            h_flex()
                .gap_2()
                .child(axis_label("0%"))
                .child(div().flex_1())
                .child(axis_label("How deep the key is pressed"))
                .child(div().flex_1())
                .child(axis_label("100%")),
        )
}

impl Plot {
    fn new(bounds: Bounds<Pixels>) -> Self {
        Self {
            area: Bounds {
                origin: bounds.origin + point(px(INSET), px(INSET)),
                size: size(
                    bounds.size.width - px(2.0 * INSET),
                    bounds.size.height - px(2.0 * INSET),
                ),
            },
        }
    }

    fn position(self, level: f32, output: f32) -> Point<Pixels> {
        point(
            self.area.origin.x + self.area.size.width * level.clamp(0.0, 1.0),
            self.area.origin.y + self.area.size.height * (1.0 - output.clamp(0.0, 1.0)),
        )
    }

    fn handle(self, point: CurvePoint) -> Point<Pixels> {
        self.position(
            f32::from(point.travel) / POINT_SCALE,
            f32::from(point.output) / POINT_SCALE,
        )
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
}

fn paint_grid(plot: Plot, colors: Colors, window: &mut Window) {
    for line in 1..GRID_LINES {
        let fraction = f32::from(line) / f32::from(GRID_LINES);
        let vertical = plot.position(fraction, 1.0);
        window.paint_quad(fill(
            Bounds {
                origin: vertical,
                size: size(px(1.0), plot.area.size.height),
            },
            colors.grid,
        ));
        let horizontal = plot.position(0.0, fraction);
        window.paint_quad(fill(
            Bounds {
                origin: horizontal,
                size: size(plot.area.size.width, px(1.0)),
            },
            colors.grid,
        ));
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
    let mut line = PathBuilder::stroke(px(2.0));
    let mut points = samples.iter();
    if let Some(first) = points.next() {
        line.move_to(*first);
    }
    for sample in points {
        line.line_to(*sample);
    }
    if let Ok(path) = line.build() {
        window.paint_path(path, colors.line);
    }
}

fn listen(plot: Plot, curve: Curve, hitbox: &Hitbox, handler: &Handler, window: &mut Window) {
    window.set_cursor_style(CursorStyle::PointingHand, hitbox);
    let grips: Vec<Point<Pixels>> = curve.iter().map(|point| plot.handle(*point)).collect();
    let hovered = hitbox.clone();
    let grab = handler.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !hovered.is_hovered(window)
        {
            return;
        }
        let nearest = grips
            .iter()
            .enumerate()
            .map(|(index, grip)| (index, distance(*grip, event.position)))
            .filter(|(_, distance)| *distance <= GRAB_DISTANCE)
            .min_by(|left, right| left.1.total_cmp(&right.1));
        if let Some((index, _)) = nearest {
            grab(Input::Grab(index), window, cx);
            cx.stop_propagation();
        }
    });
    let drag = handler.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.pressed_button == Some(MouseButton::Left) {
            let (travel, output) = plot.values(event.position);
            drag(Input::Move { travel, output }, window, cx);
        }
    });
    let release = handler.clone();
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
            release(Input::Release, window, cx);
        }
    });
}

fn distance(from: Point<Pixels>, to: Point<Pixels>) -> f32 {
    let across = f32::from(from.x - to.x);
    let down = f32::from(from.y - to.y);
    across.hypot(down)
}
