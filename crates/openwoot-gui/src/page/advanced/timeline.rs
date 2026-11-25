use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, ElementId, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        slider::Slider,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, DynamicKeystroke, Stroke, StrokeEvent},
    keys::Distance,
};

use super::{
    ActionSlot, AdvancedPage, DEFAULT_BOTTOM_OUT_SHARE,
    editor::{millimetres, slider_with_value},
    kind::{self, Point},
};
use crate::{dropdown::Dropdown, panel, store::Keyboard, theme};

const COLUMN: f32 = 72.0;
const ROW: f32 = 60.0;
const BINDING: f32 = 44.0;
const BAR: f32 = 22.0;
const DOT: f32 = 20.0;
const TARGET: f32 = 30.0;
const POINTS: usize = 4;
const BOTTOM_OUT_WIDTH: f32 = 300.0;
const INSTRUCTIONS: &str = "Drag a key from the list onto a binding. Click a plus for a single \
                            tap, or drag from one plus to another to hold the binding between \
                            them.";

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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(DOT))
            .rounded_full()
            .bg(cx.theme().primary)
            .border_2()
            .border_color(theme::keycaps().legend)
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
        let muted = cx.theme().muted_foreground;
        let mut rows: Vec<Div> = Vec::with_capacity(keystroke.strokes.len());
        for (row, stroke) in keystroke.strokes.iter().enumerate() {
            rows.push(
                h_flex()
                    .h(px(ROW))
                    .gap_4()
                    .items_center()
                    .child(self.binding_slot(
                        ActionSlot::Stroke(row),
                        stroke.action,
                        px(BINDING),
                        cx,
                    ))
                    .child(Self::lane(row, *stroke, cx)),
            );
        }
        v_flex()
            .gap_3()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Configure bindings"),
                    )
                    .child(div().text_sm().text_color(muted).child(INSTRUCTIONS)),
            )
            .child(
                v_flex()
                    .pt_2()
                    .child(self.timeline_header(entry, keystroke, keyboard, cx))
                    .children(rows),
            )
            .when(
                keystroke
                    .strokes
                    .iter()
                    .any(|stroke| kind::follows_travel(stroke.action)),
                |this| {
                    this.child(panel::note(
                        "Analog gamepad actions follow the whole key travel",
                        cx,
                    ))
                },
            )
    }

    fn timeline_header(
        &self,
        entry: &advanced::Key,
        keystroke: DynamicKeystroke,
        keyboard: &Keyboard,
        cx: &mut Context<Self>,
    ) -> Div {
        let (press, release, muted) = {
            let theme = cx.theme();
            (theme.primary, theme.success, theme.muted_foreground)
        };
        let colors = column_colors(cx);
        let key = usize::from(entry.key.get());
        let actuation = self.actuation_mm(key, keyboard);
        let bottom = keystroke.bottom_out.map_or_else(
            || keyboard.travel_mm(key) * DEFAULT_BOTTOM_OUT_SHARE,
            |distance| keyboard.distance_mm(key, distance),
        );
        let custom = keystroke.bottom_out.is_some();
        let icons = [
            Catalog::ArrowDown,
            Catalog::ArrowDownToLine,
            Catalog::ArrowUpFromLine,
            Catalog::ArrowUp,
        ];
        let mut columns: Vec<Div> = Vec::with_capacity(POINTS);
        for point in 0..POINTS {
            let color = colors.get(point).copied().unwrap_or(press);
            let label: AnyElement = if point == 0 || point == POINTS - 1 {
                div()
                    .id(SharedString::from(format!("dks-actuation-{point}")))
                    .text_xs()
                    .text_color(muted)
                    .child(SharedString::from(
                        actuation.map_or_else(|| "—".to_owned(), millimetres),
                    ))
                    .tooltip(|window, cx| {
                        Tooltip::new("The key's actuation point, set on the performance pages")
                            .build(window, cx)
                    })
                    .into_any_element()
            } else {
                self.bottom_out_label(point, bottom, custom, cx)
            };
            let icon = icons.get(point).copied().unwrap_or(Catalog::ArrowDown);
            let help = Point::ALL.get(point).map_or("", |point| point.help());
            columns.push(
                v_flex()
                    .w(px(COLUMN))
                    .gap_1()
                    .items_center()
                    .child(label)
                    .child(
                        div()
                            .id(SharedString::from(format!("dks-point-{point}")))
                            .child(Icon::new(icon).small().text_color(color))
                            .tooltip(move |window, cx| Tooltip::new(help).build(window, cx)),
                    ),
            );
        }

        v_flex()
            .pl(px(BINDING + 16.0))
            .gap_1()
            .child(
                h_flex()
                    .child(
                        div()
                            .w(px(COLUMN * 2.0))
                            .text_center()
                            .text_sm()
                            .text_color(press)
                            .child("Key press"),
                    )
                    .child(
                        div()
                            .w(px(COLUMN * 2.0))
                            .text_center()
                            .text_sm()
                            .text_color(release)
                            .child("Key release"),
                    ),
            )
            .child(h_flex().children(columns))
    }

    fn bottom_out_label(
        &self,
        point: usize,
        bottom: f32,
        custom: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let (foreground, muted) = (theme.foreground, theme.muted_foreground);
        let value = SharedString::from(millimetres(bottom));
        let slider = self.bottom_out.clone();
        let page = cx.entity().downgrade();
        let current = self.bottom_out.read(cx).value().end();
        Dropdown::new(SharedString::from(format!("dks-bottom-out-{point}")))
            .menu_width(px(BOTTOM_OUT_WIDTH))
            .trigger(move |open, _| {
                div()
                    .text_xs()
                    .underline()
                    .text_color(if open { foreground } else { muted })
                    .child(value)
                    .into_any_element()
            })
            .content(move |close, _, cx| {
                let close = close.clone();
                let page = page.clone();
                v_flex()
                    .p_3()
                    .gap_3()
                    .child(div().text_sm().child(
                        "Where the key counts as fully pressed, for the second and third points",
                    ))
                    .child(slider_with_value(
                        Slider::new(&slider),
                        millimetres(current),
                        cx,
                    ))
                    .when(custom, |this| {
                        this.child(
                            Button::new("dks-bottom-out-default")
                                .small()
                                .ghost()
                                .label("Use the default")
                                .on_click(move |_, window, cx| {
                                    page.update(cx, |page, cx| {
                                        page.set_bottom_out(None, window, cx);
                                    })
                                    .ok();
                                    close.close(cx);
                                }),
                        )
                    })
                    .into_any_element()
            })
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
        let (accent, border) = {
            let theme = cx.theme();
            (theme.primary, theme.border)
        };
        let caps = theme::keycaps();
        let colors = column_colors(cx);
        let segments = segments(stroke);
        let covered = covered(&segments);

        let dragging = cx.has_active_drag();
        let mut lane = div()
            .id(SharedString::from(format!("dks-lane-{row}")))
            .relative()
            .w(px(COLUMN * 4.0))
            .h(px(ROW));
        for (point, color) in colors.iter().enumerate() {
            lane = lane.child(
                div()
                    .absolute()
                    .left(px(centre(point) - 1.0))
                    .top_0()
                    .w(px(2.0))
                    .h_full()
                    .bg(*color),
            );
        }
        for segment in &segments {
            lane = lane.child(Self::segment(row, *segment, caps.legend, cx));
        }
        for point in 0..POINTS {
            if covered.get(point).copied().unwrap_or(true) {
                continue;
            }
            lane = lane.child(
                div()
                    .id(ElementId::Name(format!("dks-plus-{row}-{point}").into()))
                    .absolute()
                    .left(px(centre(point) - DOT / 2.0))
                    .top(px((ROW - DOT) / 2.0))
                    .size(px(DOT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(caps.top)
                    .border_1()
                    .border_color(border)
                    .text_color(caps.legend)
                    .cursor_pointer()
                    .hover(move |style| style.border_color(accent))
                    .child(Icon::new(Catalog::Plus).xsmall())
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
                        .left(px(centre(point) - TARGET / 2.0))
                        .top(px((ROW - TARGET) / 2.0))
                        .size(px(TARGET))
                        .rounded_full()
                        .drag_over::<Reach>(move |style, _, _, _| {
                            style
                                .bg(accent.opacity(0.35))
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

    fn segment(row: usize, segment: Segment, ink: Hsla, cx: &mut Context<Self>) -> AnyElement {
        let surface = theme::keycaps().top;
        match segment {
            Segment::Tap(point) => div()
                .id(ElementId::Name(format!("dks-tap-{row}-{point}").into()))
                .absolute()
                .left(px(centre(point) - DOT / 2.0))
                .top(px((ROW - DOT) / 2.0))
                .size(px(DOT))
                .rounded_full()
                .bg(ink)
                .cursor_pointer()
                .tooltip(|window, cx| Tooltip::new("Tap; click to remove").build(window, cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_point(row, point, StrokeEvent::Nothing, window, cx);
                }))
                .into_any_element(),
            Segment::Release(point) => div()
                .id(ElementId::Name(format!("dks-release-{row}-{point}").into()))
                .absolute()
                .left(px(centre(point) - 3.0))
                .top(px((ROW - BAR) / 2.0))
                .w(px(6.0))
                .h(px(BAR))
                .rounded(px(3.0))
                .bg(ink)
                .cursor_pointer()
                .tooltip(|window, cx| {
                    Tooltip::new("Release without a press; click to remove").build(window, cx)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_point(row, point, StrokeEvent::Nothing, window, cx);
                }))
                .into_any_element(),
            Segment::Hold(start, end) => {
                let left = centre(start) - BAR / 2.0;
                let right = end.map_or(COLUMN * 4.0 - 4.0, |end| centre(end) + BAR / 2.0);
                let group = SharedString::from(format!("dks-hold-{row}-{start}"));
                div()
                    .id(group.clone())
                    .group(group.clone())
                    .absolute()
                    .left(px(left))
                    .top(px((ROW - BAR) / 2.0))
                    .w(px(right - left))
                    .h(px(BAR))
                    .rounded_full()
                    .bg(ink)
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
                            .right(px(4.0))
                            .top(px(4.0))
                            .w(px(4.0))
                            .h(px(BAR - 8.0))
                            .rounded(px(2.0))
                            .bg(surface)
                            .cursor_ew_resize()
                            .tooltip(|window, cx| {
                                Tooltip::new("Drag to change where it ends").build(window, cx)
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
            .left(px(BAR / 2.0 - 7.0))
            .top(px((BAR - 14.0) / 2.0))
            .size(px(14.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(danger)
            .text_color(surface)
            .invisible()
            .group_hover(group, gpui_kit::Styled::visible)
            .cursor_pointer()
            .child(Icon::new(Catalog::Close).xsmall())
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
            StrokeEvent::Tap => found.push(Segment::Tap(point)),
            StrokeEvent::Press => {
                if let Some(start) = open.replace(point) {
                    found.push(Segment::Hold(start, None));
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
            Segment::Tap(point) | Segment::Release(point) => (point, point),
            Segment::Hold(start, end) => (start, end.unwrap_or(POINTS - 1)),
        };
        for slot in covered.iter_mut().take(to + 1).skip(from) {
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

fn centre(point: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a point is one of four timeline columns"
    )]
    let point = point as f32;
    (point + 0.5) * COLUMN
}

fn column_colors(cx: &Context<AdvancedPage>) -> [Hsla; POINTS] {
    let theme = cx.theme();
    [
        theme.primary,
        theme.primary.blend(theme.danger.opacity(0.55)),
        theme.success,
        theme.success.blend(theme.primary.opacity(0.25)),
    ]
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
    fn a_hold_covers_every_point_it_spans() {
        assert_eq!(
            covered(&[Segment::Hold(0, Some(2))]),
            [true, true, true, false]
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
