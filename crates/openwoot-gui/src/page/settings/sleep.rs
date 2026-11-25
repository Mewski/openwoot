use std::rc::Rc;

use gpui_kit::{
    App, AppContext as _, Context, Div, DragMoveEvent, Empty, FontWeight, InteractiveElement as _,
    IntoElement, MouseButton, ParentElement as _, Pixels, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, v_flex},
    div,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::lighting::Idle;

const STOPS: [Option<u8>; 8] = [
    Some(1),
    Some(3),
    Some(5),
    Some(10),
    Some(20),
    Some(30),
    Some(60),
    None,
];
const NEVER: u8 = 7;
const SMALL_SIZE: f32 = 14.0;
const FIELD_LINE: f32 = 16.8;
const FIELD_GAP: f32 = 8.0;
const THUMB: f32 = 16.0;
const TRACK: f32 = 4.0;
const TICK: f32 = 2.0;
const STOP_INSET: f32 = THUMB / 2.0;
const STOP_LABEL_LINE: f32 = 14.0;
const STOP_LABEL_GAP: f32 = 4.0;
const STOP_LABEL_SLOT: f32 = 48.0;
const CAPTION_LINE: f32 = 21.0;
const CAPTION_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Sleep {
    pub(super) dim: u8,
    pub(super) off: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    Move(u8),
    Release,
}

#[derive(Clone, Copy)]
struct StopDrag(&'static str);

impl Sleep {
    pub(super) fn of(idle: Option<Idle>) -> Self {
        let Some(idle) = idle else {
            return Self {
                dim: NEVER,
                off: NEVER,
            };
        };
        let off = if idle.off_enabled {
            nearest(idle.off_after_minutes)
        } else {
            NEVER
        };
        Self {
            dim: nearest(idle.dim_after_minutes).min(off),
            off,
        }
    }

    pub(super) fn idle(self) -> Option<Idle> {
        let dim_after_minutes = minutes(self.dim)?;
        let off = minutes(self.off);
        Some(Idle {
            dim_after_minutes,
            off_after_minutes: off.unwrap_or(dim_after_minutes),
            off_enabled: off.is_some(),
        })
    }

    pub(super) fn with_dim(self, dim: u8) -> Self {
        let dim = dim.min(NEVER);
        Self {
            dim,
            off: self.off.max(dim),
        }
    }

    pub(super) fn with_off(self, off: u8) -> Self {
        let off = off.min(NEVER);
        Self {
            dim: self.dim.min(off),
            off,
        }
    }
}

pub(super) fn stepped(
    id: &'static str,
    title: &'static str,
    current: u8,
    disabled: bool,
    on_step: impl Fn(&Step, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_step: OnStep = Rc::new(on_step);
    v_flex()
        .w_full()
        .gap(px(FIELD_GAP))
        .child(
            div()
                .text_size(px(SMALL_SIZE))
                .line_height(px(FIELD_LINE))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.group_box_foreground)
                .child(title),
        )
        .child(
            v_flex()
                .w_full()
                .child(track(id, current, disabled, &on_step, cx))
                .child(
                    div()
                        .w_full()
                        .mt(px(CAPTION_GAP))
                        .text_center()
                        .text_size(px(SMALL_SIZE))
                        .line_height(px(CAPTION_LINE))
                        .text_color(theme.group_box_foreground)
                        .child("Minutes since last key press"),
                ),
        )
}

impl Render for StopDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

type OnStep = Rc<dyn Fn(&Step, &mut Window, &mut App)>;

fn track(id: &'static str, current: u8, disabled: bool, on_step: &OnStep, cx: &App) -> Div {
    let theme = cx.theme();
    let last = f32::from(NEVER);
    let at = |stop: u8| relative(f32::from(stop.min(NEVER)) / last);
    let ticks = (0..=NEVER).map(|stop| {
        div()
            .absolute()
            .top(px((THUMB - TICK) / 2.0))
            .left(at(stop))
            .ml(px(-TICK / 2.0))
            .size(px(TICK))
            .rounded_full()
            .bg(theme.background)
    });
    let zones = (0..=NEVER).filter(|_| !disabled).map(|stop| {
        let on_step = on_step.clone();
        div()
            .id((id, usize::from(stop)))
            .absolute()
            .top_0()
            .h_full()
            .left(relative((f32::from(stop) - 0.5) / last))
            .w(relative(1.0 / last))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                on_step(&Step::Move(stop), window, cx);
            })
    });
    let rail = div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(STOP_INSET))
        .right(px(STOP_INSET))
        .child(
            div()
                .absolute()
                .top(px((THUMB - TRACK) / 2.0))
                .h(px(TRACK))
                .left(px(-STOP_INSET))
                .right(relative(1.0 - f32::from(current.min(NEVER)) / last))
                .rounded_full()
                .bg(theme.primary),
        )
        .children(ticks)
        .child(
            div()
                .absolute()
                .top_0()
                .left(at(current))
                .ml(px(-THUMB / 2.0))
                .size(px(THUMB))
                .rounded_full()
                .border_1()
                .border_color(theme.primary)
                .bg(theme.background),
        )
        .children(zones);
    let release = on_step.clone();
    let release_out = on_step.clone();
    let moved = on_step.clone();
    v_flex()
        .w_full()
        .child(
            div()
                .id(SharedString::from(format!("{id}-track")))
                .relative()
                .w_full()
                .h(px(THUMB))
                .child(
                    div()
                        .absolute()
                        .top(px((THUMB - TRACK) / 2.0))
                        .left_0()
                        .right_0()
                        .h(px(TRACK))
                        .rounded_full()
                        .bg(theme.secondary_active),
                )
                .child(rail)
                .when(!disabled, |this| {
                    this.on_drag(StopDrag(id), |drag, _, _, cx| cx.new(|_| *drag))
                        .on_drag_move(move |event: &DragMoveEvent<StopDrag>, window, cx| {
                            if event.drag(cx).0 == id {
                                let offset = event.event.position.x - event.bounds.origin.x;
                                moved(
                                    &Step::Move(stop_at(offset, event.bounds.size.width)),
                                    window,
                                    cx,
                                );
                            }
                        })
                        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                            release(&Step::Release, window, cx);
                        })
                        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                            release_out(&Step::Release, window, cx);
                        })
                }),
        )
        .child(stop_labels(id, current, disabled, on_step, cx))
}

fn stop_labels(id: &'static str, current: u8, disabled: bool, on_step: &OnStep, cx: &App) -> Div {
    let theme = cx.theme();
    let last = f32::from(NEVER);
    let hover = theme.foreground;
    let labels = (0..=NEVER).map(|stop| {
        let on_step = on_step.clone();
        let chosen = stop == current;
        div()
            .id(SharedString::from(format!("{id}-label-{stop}")))
            .absolute()
            .top_0()
            .flex()
            .map(|this| {
                if stop == NEVER {
                    this.right(px(-STOP_INSET)).justify_end()
                } else {
                    this.left(relative(f32::from(stop) / last))
                        .ml(px(-STOP_LABEL_SLOT / 2.0))
                        .w(px(STOP_LABEL_SLOT))
                        .justify_center()
                }
            })
            .text_size(px(SMALL_SIZE))
            .line_height(px(STOP_LABEL_LINE))
            .text_color(if chosen {
                theme.foreground
            } else {
                theme.sidebar_foreground
            })
            .when(!disabled && !chosen, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.text_color(hover))
                    .on_click(move |_, window, cx| {
                        on_step(&Step::Move(stop), window, cx);
                        on_step(&Step::Release, window, cx);
                    })
            })
            .child(label(stop))
    });
    div()
        .relative()
        .w_full()
        .mt(px(STOP_LABEL_GAP))
        .h(px(STOP_LABEL_LINE))
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(STOP_INSET))
                .right(px(STOP_INSET))
                .children(labels),
        )
}

fn stop_at(offset: Pixels, width: Pixels) -> u8 {
    let span = f32::from(width) - THUMB;
    let position = if span > 0.0 {
        (f32::from(offset) - STOP_INSET) / span * f32::from(NEVER)
    } else {
        0.0
    };
    (0..=NEVER)
        .min_by(|left, right| {
            (position - f32::from(*left))
                .abs()
                .total_cmp(&(position - f32::from(*right)).abs())
        })
        .unwrap_or(NEVER)
}

fn label(stop: u8) -> SharedString {
    minutes(stop).map_or_else(
        || SharedString::from("Never"),
        |minutes| format!("{minutes}’").into(),
    )
}

fn minutes(stop: u8) -> Option<u8> {
    STOPS.get(usize::from(stop)).copied().flatten()
}

fn nearest(target: u8) -> u8 {
    (0..NEVER)
        .filter_map(|stop| minutes(stop).map(|minutes| (stop, minutes.abs_diff(target))))
        .min_by_key(|(_, distance)| *distance)
        .map_or(NEVER, |(stop, _)| stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle(dim: u8, off: u8, off_enabled: bool) -> Idle {
        Idle {
            dim_after_minutes: dim,
            off_after_minutes: off,
            off_enabled,
        }
    }

    #[test]
    fn no_idle_reads_as_never_for_both() {
        assert_eq!(Sleep::of(None), Sleep { dim: 7, off: 7 });
        assert_eq!(Sleep { dim: 7, off: 7 }.idle(), None);
    }

    #[test]
    fn idle_minutes_snap_to_the_nearest_stop() {
        assert_eq!(Sleep::of(Some(idle(5, 15, true))), Sleep { dim: 2, off: 3 });
        assert_eq!(
            Sleep::of(Some(idle(45, 200, true))),
            Sleep { dim: 5, off: 6 }
        );
    }

    #[test]
    fn turning_off_disabled_reads_as_never() {
        assert_eq!(
            Sleep::of(Some(idle(10, 1, false))),
            Sleep { dim: 3, off: 7 }
        );
        assert_eq!(Sleep { dim: 3, off: 7 }.idle(), Some(idle(10, 10, false)));
    }

    #[test]
    fn dimming_never_comes_after_turning_off() {
        assert_eq!(Sleep::of(Some(idle(30, 5, true))), Sleep { dim: 2, off: 2 });
        let sleep = Sleep { dim: 2, off: 4 };
        assert_eq!(sleep.with_dim(6), Sleep { dim: 6, off: 6 });
        assert_eq!(sleep.with_off(1), Sleep { dim: 1, off: 1 });
        assert_eq!(sleep.with_off(7), Sleep { dim: 2, off: 7 });
    }

    #[test]
    fn stops_write_their_minutes() {
        assert_eq!(Sleep { dim: 1, off: 4 }.idle(), Some(idle(3, 20, true)));
    }

    #[test]
    fn track_positions_round_to_the_nearest_stop() {
        let width = px(580.0);
        assert_eq!(stop_at(px(8.0), width), 0);
        assert_eq!(stop_at(px(-40.0), width), 0);
        assert_eq!(stop_at(px(170.0), width), 2);
        assert_eq!(stop_at(px(572.0), width), 7);
        assert_eq!(stop_at(px(900.0), width), 7);
        assert_eq!(stop_at(px(100.0), px(10.0)), 0);
    }

    #[test]
    fn labels_name_minutes_and_never() {
        assert_eq!(label(0), "1’");
        assert_eq!(label(6), "60’");
        assert_eq!(label(7), "Never");
    }
}
