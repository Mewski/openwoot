use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, ElementId, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, rgb,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, DynamicKeystroke, Stroke, StrokeEvent},
    keys::Distance,
};

use super::{
    ActionSlot, AdvancedPage, DEFAULT_BOTTOM_OUT_SHARE,
    kind::{self, Point},
};
use crate::{
    page::performance::{DistanceField, art_slot, switch},
    panel,
    store::Keyboard,
    theme,
};

const POINTS: usize = 4;
const STACK_GAP: f32 = 24.0;
const CONFIGURE_TEXT: f32 = 12.8;
const CONFIGURE_LINE: f32 = 15.36;
const GRID_WIDTH: f32 = 424.0;
const GRID_HEIGHT: f32 = 366.0;
const SLOT: f32 = 44.0;
const GROUP_LEFT: [f32; 2] = [80.0, 256.0];
const GROUP_WIDTH: f32 = 168.0;
const GROUP_RADIUS: f32 = 6.0;
const POINT_X: [f32; POINTS] = [121.0, 201.0, 301.0, 381.0];
const GROUP_TITLE_TOP: f32 = 8.0;
const SMALL_TEXT: f32 = 12.8;
const SMALL_LINE: f32 = 19.2;
const DISTANCE_TOP: f32 = 36.0;
const DISTANCE_WIDTH: f32 = 46.0;
const DISTANCE_TEXT: f32 = 12.0;
const ICON_TOP: f32 = 62.0;
const ICON_BOX: f32 = 30.0;
const ICON: f32 = 20.0;
const LABEL_TOP: f32 = 87.0;
const LINE_TOP: f32 = 112.0;
const LINE_HEIGHT: f32 = 230.0;
const LINE_WIDTH: f32 = 2.0;
const ROW_TOP: f32 = 122.0;
const ROW_PITCH: f32 = 60.0;
const LANE_LEFT: f32 = 112.0;
const LANE_WIDTH: f32 = 290.0;
const PLUS: f32 = 24.0;
const PLUS_TOP: f32 = 7.0;
const PLUS_ICON: f32 = 14.0;
const BAR_TOP: f32 = 6.0;
const BAR_HEIGHT: f32 = 25.0;
const BAR_LEAD: f32 = 13.0;
const BAR_TAIL: f32 = 15.0;
const HANDLE_WIDTH: f32 = 4.0;
const HANDLE_INSET: f32 = 6.0;
const REMOVE: f32 = 14.0;
const TARGET: f32 = 30.0;
const DROP_TINT: f32 = 0.35;
const MODAL_WIDTH: f32 = 512.0;
const MODAL_GAP: f32 = 8.0;
const MODAL_SPACING: f32 = 32.0;
const MODAL_ART_BOTTOM: f32 = 16.0;
const MODAL_TEXT_WIDTH: f32 = 212.0;
const MODAL_ICONS_TOP: f32 = 16.0;
const POINT_COLORS: [u32; POINTS] = [0x00fb_d38d, 0x00ed_8936, 0x005c_d18b, 0x00a3_e5be];
const PRESS_TITLE: u32 = 0x00f6_ad55;
const RELEASE_TITLE: u32 = 0x0080_dba5;
const HOLD_FILL: u32 = 0x00c4_c4c4;
const DESCRIPTION: &str = "Assign up to four bindings to each of the four parts of a key press.";
const CONFIGURE: &str = "Configure DKS Bindings";
const CONFIGURE_DESCRIPTION: &str = "Drag and drop a binding from the list to the desired action.";
const PLUS_HELP: &str = "Click for a single activation or drag across plus icons to hold.";
const ACTUATION_HELP: &str = "Set on the Actuation Point page.";
const BOTTOM_OUT_HELP: &str = "Set the actuation point for the second and third DKS action.";
const ANALOG: &str = "Analog gamepad bindings are always active across the entire key range.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Reach {
    row: usize,
    from: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Segment {
    Tap(usize),
    Hold(usize, Option<usize>),
    Release(usize),
}

impl Render for Reach {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(BAR_HEIGHT))
            .rounded_full()
            .bg(rgb(HOLD_FILL))
            .border_1()
            .border_color(theme::palette().heading)
    }
}

impl AdvancedPage {
    pub(super) fn timeline(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let heading = theme::palette().heading;
        let mut grid = div()
            .relative()
            .flex_none()
            .w(px(GRID_WIDTH))
            .h(px(GRID_HEIGHT))
            .children(groups())
            .children(self.distances(entry, keystroke, keyboard, cx))
            .children(point_icons())
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(LABEL_TOP))
                    .text_size(px(SMALL_TEXT))
                    .line_height(px(SMALL_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::keycaps().legend)
                    .child("Bindings"),
            )
            .children(POINT_X.iter().zip(POINT_COLORS).map(|(x, color)| {
                div()
                    .absolute()
                    .left(px(x - LINE_WIDTH / 2.0))
                    .top(px(LINE_TOP))
                    .w(px(LINE_WIDTH))
                    .h(px(LINE_HEIGHT))
                    .bg(rgb(color))
            }));
        for (row, stroke) in keystroke.strokes.iter().enumerate() {
            let top = ROW_TOP + ROW_PITCH * f32_of(row);
            grid = grid
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(top))
                        .child(self.binding_slot(
                            ActionSlot::Stroke(row),
                            stroke.action,
                            px(SLOT),
                            cx,
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(LANE_LEFT))
                        .top(px(top))
                        .child(Self::lane(row, *stroke, cx)),
                );
        }
        v_flex()
            .gap(px(STACK_GAP))
            .child(panel::description(DESCRIPTION, cx))
            .child(
                v_flex()
                    .child(
                        div()
                            .text_size(px(CONFIGURE_TEXT))
                            .line_height(px(CONFIGURE_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(heading)
                            .child(CONFIGURE),
                    )
                    .child(panel::description(CONFIGURE_DESCRIPTION, cx)),
            )
            .child(grid)
            .when(
                keystroke
                    .strokes
                    .iter()
                    .any(|stroke| kind::follows_travel(stroke.action)),
                |this| this.child(panel::note(ANALOG, cx)),
            )
    }

    fn distances(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let key = usize::from(entry.key.get());
        let actuation = self.actuation_mm(key, keyboard);
        let bottom = keystroke.bottom_out.map_or_else(
            || keyboard.travel_mm(key) * DEFAULT_BOTTOM_OUT_SHARE,
            |distance| keyboard.distance_mm(key, distance),
        );
        let muted = cx.theme().sidebar_foreground;
        (0..POINTS)
            .map(|point| {
                let x = POINT_X.get(point).copied().unwrap_or_default();
                let label = if point == 0 || point == POINTS - 1 {
                    div()
                        .id(SharedString::from(format!("dks-actuation-{point}")))
                        .text_size(px(DISTANCE_TEXT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .underline()
                        .text_color(muted)
                        .child(SharedString::from(
                            actuation.map_or_else(|| "—".to_owned(), millimetres),
                        ))
                        .tooltip(|window, cx| Tooltip::new(ACTUATION_HELP).build(window, cx))
                        .into_any_element()
                } else {
                    self.bottom_out_label(point, bottom, cx)
                };
                h_flex()
                    .absolute()
                    .left(px(x - DISTANCE_WIDTH / 2.0))
                    .top(px(DISTANCE_TOP))
                    .w(px(DISTANCE_WIDTH))
                    .justify_center()
                    .child(label)
                    .into_any_element()
            })
            .collect()
    }

    fn bottom_out_label(&self, point: usize, bottom: f32, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().sidebar_foreground;
        let field = self.bottom_out.clone();
        div()
            .id(SharedString::from(format!("dks-bottom-out-{point}")))
            .text_size(px(DISTANCE_TEXT))
            .font_weight(FontWeight::SEMIBOLD)
            .underline()
            .text_color(muted)
            .cursor_pointer()
            .child(SharedString::from(millimetres(bottom)))
            .on_click(move |_, window, cx| bottom_out_modal(&field).open(window, cx))
            .into_any_element()
    }

    fn actuation_mm(&self, key: usize, keyboard: &Keyboard) -> Option<f32> {
        let settings = self.settings?;
        let distance: Distance = self
            .overrides
            .as_ref()
            .and_then(|overrides| overrides.get(key))
            .and_then(|key_override| key_override.actuation)
            .unwrap_or(settings.actuation);
        Some(keyboard.distance_mm(key, distance))
    }

    fn lane(row: usize, stroke: Stroke, cx: &mut Context<Self>) -> Stateful<Div> {
        let (accent, plus_fill, plus_hover, plus_ink) = {
            let theme = cx.theme();
            let palette = theme::palette();
            (
                theme.primary,
                palette.raised,
                palette.raised_hover,
                theme.foreground,
            )
        };
        let segments = segments(stroke);
        let covered = covered(&segments);
        let dragging = cx.has_active_drag();
        let mut lane = div()
            .id(SharedString::from(format!("dks-lane-{row}")))
            .relative()
            .w(px(LANE_WIDTH))
            .h(px(SLOT));
        for segment in &segments {
            lane = lane.child(Self::segment(row, *segment, &segments, cx));
        }
        for point in 0..POINTS {
            if covered.get(point).copied().unwrap_or(true) {
                continue;
            }
            lane = lane.child(
                div()
                    .id(ElementId::Name(format!("dks-plus-{row}-{point}").into()))
                    .absolute()
                    .left(px(lane_x(point) - PLUS / 2.0))
                    .top(px(PLUS_TOP))
                    .size(px(PLUS))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(plus_fill)
                    .text_color(plus_ink)
                    .cursor_pointer()
                    .hover(move |style| style.bg(plus_hover))
                    .child(Icon::new(Catalog::Plus).size(px(PLUS_ICON)))
                    .tooltip(|window, cx| Tooltip::new(PLUS_HELP).build(window, cx))
                    .on_drag(Reach { row, from: point }, |reach, _, _, cx| {
                        cx.new(|_| *reach)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_point(row, point, StrokeEvent::Tap, window, cx);
                    })),
            );
        }
        if dragging {
            for point in 0..POINTS {
                lane = lane.child(
                    div()
                        .id(ElementId::Name(format!("dks-target-{row}-{point}").into()))
                        .absolute()
                        .left(px(lane_x(point) - TARGET / 2.0))
                        .top(px(PLUS_TOP + PLUS / 2.0 - TARGET / 2.0))
                        .size(px(TARGET))
                        .rounded_full()
                        .drag_over::<Reach>(move |style, _, _, _| {
                            style
                                .bg(accent.opacity(DROP_TINT))
                                .border_2()
                                .border_color(accent)
                        })
                        .on_drop(cx.listener(move |this, reach: &Reach, window, cx| {
                            this.reach(*reach, row, point, window, cx);
                        })),
                );
            }
        }
        lane
    }

    fn segment(
        row: usize,
        segment: Segment,
        segments: &[Segment],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let edge = theme::palette().heading;
        let fill: Hsla = rgb(HOLD_FILL).into();
        match segment {
            Segment::Tap(point) => {
                let ends_hold = segments
                    .iter()
                    .any(|other| matches!(other, Segment::Hold(_, Some(end)) if *end == point));
                let cleared = if ends_hold {
                    StrokeEvent::Release
                } else {
                    StrokeEvent::Nothing
                };
                div()
                    .id(ElementId::Name(format!("dks-tap-{row}-{point}").into()))
                    .absolute()
                    .left(px(lane_x(point) - BAR_HEIGHT / 2.0))
                    .top(px(BAR_TOP))
                    .size(px(BAR_HEIGHT))
                    .rounded_full()
                    .bg(fill)
                    .border_1()
                    .border_color(edge)
                    .cursor_pointer()
                    .tooltip(|window, cx| {
                        Tooltip::new("Click to remove the tap.").build(window, cx)
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_point(row, point, cleared, window, cx);
                    }))
                    .into_any_element()
            }
            Segment::Release(point) => div()
                .id(ElementId::Name(format!("dks-release-{row}-{point}").into()))
                .absolute()
                .left(px(lane_x(point) - HANDLE_WIDTH))
                .top(px(BAR_TOP))
                .w(px(HANDLE_WIDTH * 2.0))
                .h(px(BAR_HEIGHT))
                .rounded_full()
                .bg(fill)
                .cursor_pointer()
                .tooltip(|window, cx| {
                    Tooltip::new("Click to remove the release.").build(window, cx)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_point(row, point, StrokeEvent::Nothing, window, cx);
                }))
                .into_any_element(),
            Segment::Hold(start, end) => {
                let left = lane_x(start) - BAR_LEAD;
                let right = lane_x(end.unwrap_or(POINTS - 1)) - BAR_TAIL;
                let group = SharedString::from(format!("dks-hold-{row}-{start}"));
                div()
                    .id(group.clone())
                    .group(group.clone())
                    .absolute()
                    .left(px(left))
                    .top(px(BAR_TOP))
                    .w(px(right - left))
                    .h(px(BAR_HEIGHT))
                    .rounded_full()
                    .bg(fill)
                    .border_1()
                    .border_color(edge)
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, _, window, cx| {
                            this.clear_hold(row, start, window, cx);
                        }),
                    )
                    .child(Self::hold_remove(row, start, group, cx))
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("dks-hold-end-{row}-{start}").into(),
                            ))
                            .absolute()
                            .right(px(HANDLE_INSET))
                            .top(px(HANDLE_INSET))
                            .w(px(HANDLE_WIDTH))
                            .h(px(BAR_HEIGHT - 2.0 * HANDLE_INSET))
                            .rounded_full()
                            .bg(theme::keycaps().top)
                            .cursor_ew_resize()
                            .tooltip(|window, cx| {
                                Tooltip::new("Drag to change where it ends.").build(window, cx)
                            })
                            .on_drag(Reach { row, from: start }, |reach, _, _, cx| {
                                cx.new(|_| *reach)
                            }),
                    )
                    .into_any_element()
            }
        }
    }

    fn hold_remove(
        row: usize,
        start: usize,
        group: SharedString,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let (danger, surface) = (cx.theme().danger, theme::keycaps().top);
        div()
            .id(ElementId::Name(
                format!("dks-hold-remove-{row}-{start}").into(),
            ))
            .absolute()
            .left(px((BAR_HEIGHT - REMOVE) / 2.0))
            .top(px((BAR_HEIGHT - REMOVE) / 2.0 - 1.0))
            .size(px(REMOVE))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(danger)
            .text_color(surface)
            .invisible()
            .group_hover(group, gpui_kit::Styled::visible)
            .cursor_pointer()
            .child(Icon::new(Catalog::Close).size(px(REMOVE * 0.7)))
            .tooltip(|window, cx| Tooltip::new("Remove").build(window, cx))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.clear_hold(row, start, window, cx);
            }))
    }

    fn change_stroke(
        &mut self,
        row: usize,
        change: impl FnOnce(&mut [StrokeEvent; POINTS]),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_edited(
            |entry| {
                if let Behaviour::DynamicKeystroke(keystroke) = &mut entry.behaviour
                    && let Some(stroke) = keystroke.strokes.get_mut(row)
                {
                    let mut events = Point::ALL.map(|point| point.event(*stroke));
                    change(&mut events);
                    for (point, event) in Point::ALL.into_iter().zip(events) {
                        point.set(stroke, event);
                    }
                }
            },
            window,
            cx,
        );
    }

    fn set_point(
        &mut self,
        row: usize,
        point: usize,
        event: StrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_stroke(
            row,
            |events| {
                if let Some(slot) = events.get_mut(point) {
                    *slot = event;
                }
            },
            window,
            cx,
        );
    }

    fn clear_hold(
        &mut self,
        row: usize,
        start: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_stroke(row, |events| clear_hold_events(events, start), window, cx);
    }

    fn reach(
        &mut self,
        reach: Reach,
        row: usize,
        point: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if reach.row != row || point <= reach.from {
            return;
        }
        self.change_stroke(
            row,
            |events| hold_events(events, reach.from, point),
            window,
            cx,
        );
    }
}

fn groups() -> [Div; 2] {
    let tint = theme::palette().tint;
    let group = |left: f32, title: &'static str, color: u32| {
        div()
            .absolute()
            .left(px(left))
            .top_0()
            .w(px(GROUP_WIDTH))
            .h(px(GRID_HEIGHT))
            .pt(px(GROUP_TITLE_TOP))
            .rounded(px(GROUP_RADIUS))
            .bg(tint)
            .flex()
            .justify_center()
            .text_size(px(SMALL_TEXT))
            .line_height(px(SMALL_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(color))
            .child(title)
    };
    let [press, release] = GROUP_LEFT;
    [
        group(press, "Key Press", PRESS_TITLE),
        group(release, "Key Release", RELEASE_TITLE),
    ]
}

fn point_icons() -> Vec<Div> {
    (0..POINTS)
        .map(|point| {
            let x = POINT_X.get(point).copied().unwrap_or_default();
            div()
                .absolute()
                .left(px(x - ICON_BOX / 2.0))
                .top(px(ICON_TOP))
                .child(point_icon(point))
        })
        .collect()
}

fn point_icon(point: usize) -> Stateful<Div> {
    let icon = match point {
        0 => Catalog::ArrowDownToLine,
        1 => Catalog::ArrowDown,
        2 => Catalog::ArrowUp,
        _ => Catalog::ArrowUpFromLine,
    };
    let color = POINT_COLORS.get(point).copied().unwrap_or(PRESS_TITLE);
    let help = Point::ALL.get(point).map_or("", |point| point.help());
    h_flex()
        .id(SharedString::from(format!("dks-point-{point}")))
        .size(px(ICON_BOX))
        .justify_center()
        .child(Icon::new(icon).size(px(ICON)).text_color(rgb(color)))
        .tooltip(move |window, cx| Tooltip::new(help).build(window, cx))
}

fn bottom_out_modal(field: &Entity<DistanceField>) -> panel::Modal {
    let field = field.clone();
    panel::Modal::new(px(MODAL_WIDTH), move |_, cx| {
        let depth = field.read(cx).depth(cx);
        let stem = cx.theme().success;
        h_flex()
            .w_full()
            .items_start()
            .gap(px(MODAL_GAP))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .mr(px(MODAL_SPACING))
                    .mb(px(MODAL_ART_BOTTOM))
                    .items_center()
                    .gap(px(MODAL_SPACING))
                    .child(art_slot(switch(depth, stem, cx)))
                    .child(field.clone()),
            )
            .child(
                v_flex()
                    .flex_none()
                    .w(px(MODAL_TEXT_WIDTH))
                    .child(
                        div()
                            .text_size(px(SMALL_TEXT))
                            .line_height(px(SMALL_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::keycaps().legend)
                            .child(BOTTOM_OUT_HELP),
                    )
                    .child(
                        h_flex()
                            .mt(px(MODAL_ICONS_TOP))
                            .gap(px(MODAL_GAP))
                            .justify_around()
                            .children([1, 2].map(point_icon)),
                    ),
            )
            .into_any_element()
    })
}

fn millimetres(value: f32) -> String {
    format!("{value:.2}mm")
}

fn lane_x(point: usize) -> f32 {
    POINT_X.get(point).copied().unwrap_or_default() - LANE_LEFT
}

fn f32_of(row: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a row is one of four dynamic keystroke bindings"
    )]
    let row = row as f32;
    row
}

fn segments(stroke: Stroke) -> Vec<Segment> {
    let mut found = Vec::new();
    let mut open: Option<usize> = None;
    for (point, event) in Point::ALL
        .into_iter()
        .map(|point| point.event(stroke))
        .enumerate()
    {
        match event {
            StrokeEvent::Nothing => {}
            StrokeEvent::Tap => {
                if let Some(start) = open.take() {
                    found.push(Segment::Hold(start, Some(point)));
                }
                found.push(Segment::Tap(point));
            }
            StrokeEvent::Press => {
                if let Some(start) = open.replace(point) {
                    found.push(Segment::Hold(start, Some(point)));
                }
            }
            StrokeEvent::Release => match open.take() {
                Some(start) => found.push(Segment::Hold(start, Some(point))),
                None => found.push(Segment::Release(point)),
            },
        }
    }
    if let Some(start) = open {
        found.push(Segment::Hold(start, None));
    }
    found
}

fn covered(segments: &[Segment]) -> [bool; POINTS] {
    let mut covered = [false; POINTS];
    for segment in segments {
        let (from, to) = match *segment {
            Segment::Tap(point) | Segment::Release(point) => (point, point + 1),
            Segment::Hold(start, end) => (start, end.unwrap_or(POINTS - 1)),
        };
        for slot in covered.iter_mut().take(to).skip(from) {
            *slot = true;
        }
    }
    covered
}

fn hold_events(events: &mut [StrokeEvent; POINTS], from: usize, to: usize) {
    let previous_end = events
        .iter()
        .enumerate()
        .skip(from + 1)
        .find(|(_, event)| **event == StrokeEvent::Release)
        .map(|(point, _)| point);
    if events.get(from) == Some(&StrokeEvent::Press)
        && let Some(end) = previous_end
        && let Some(slot) = events.get_mut(end)
    {
        *slot = StrokeEvent::Nothing;
    }
    for (point, slot) in events.iter_mut().enumerate() {
        if point == from {
            *slot = StrokeEvent::Press;
        } else if point == to {
            *slot = StrokeEvent::Release;
        } else if point > from && point < to {
            *slot = StrokeEvent::Nothing;
        }
    }
}

fn clear_hold_events(events: &mut [StrokeEvent; POINTS], start: usize) {
    let end = events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, event)| **event == StrokeEvent::Release)
        .map(|(point, _)| point);
    if let Some(slot) = events.get_mut(start) {
        *slot = StrokeEvent::Nothing;
    }
    if let Some(end) = end
        && let Some(slot) = events.get_mut(end)
    {
        *slot = StrokeEvent::Nothing;
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Action;

    use super::*;

    fn stroke(events: [StrokeEvent; POINTS]) -> Stroke {
        let [on_press, on_bottom_out, on_lift, on_release] = events;
        Stroke {
            action: Action::Keyboard(0x04),
            on_press,
            on_bottom_out,
            on_lift,
            on_release,
        }
    }

    #[test]
    fn presses_and_releases_read_as_holds_and_taps_stay_dots() {
        use StrokeEvent::{Nothing, Press, Release, Tap};
        assert_eq!(
            segments(stroke([Press, Nothing, Nothing, Release])),
            vec![Segment::Hold(0, Some(3))]
        );
        assert_eq!(
            segments(stroke([Tap, Press, Release, Nothing])),
            vec![Segment::Tap(0), Segment::Hold(1, Some(2))]
        );
        assert_eq!(
            segments(stroke([Nothing, Press, Nothing, Nothing])),
            vec![Segment::Hold(1, None)]
        );
        assert_eq!(
            segments(stroke([Nothing, Nothing, Release, Nothing])),
            vec![Segment::Release(2)]
        );
    }

    #[test]
    fn a_tap_during_a_hold_ends_the_hold_at_that_point() {
        use StrokeEvent::{Nothing, Press, Tap};
        assert_eq!(
            segments(stroke([Press, Nothing, Nothing, Tap])),
            vec![Segment::Hold(0, Some(3)), Segment::Tap(3)]
        );
    }

    #[test]
    fn a_hold_leaves_its_end_point_free_for_a_tap() {
        assert_eq!(
            covered(&[Segment::Hold(0, Some(2))]),
            [true, true, false, false]
        );
        assert_eq!(
            covered(&[Segment::Hold(1, None)]),
            [false, true, true, false]
        );
        assert_eq!(covered(&[Segment::Tap(3)]), [false, false, false, true]);
    }

    #[test]
    fn dragging_between_points_makes_a_hold_and_moves_its_end() {
        use StrokeEvent::{Nothing, Press, Release, Tap};
        let mut events = [Nothing, Tap, Nothing, Nothing];
        hold_events(&mut events, 0, 2);
        assert_eq!(events, [Press, Nothing, Release, Nothing]);
        hold_events(&mut events, 0, 3);
        assert_eq!(events, [Press, Nothing, Nothing, Release]);
        hold_events(&mut events, 0, 1);
        assert_eq!(events, [Press, Release, Nothing, Nothing]);
        clear_hold_events(&mut events, 0);
        assert_eq!(events, [Nothing; POINTS]);
    }
}
