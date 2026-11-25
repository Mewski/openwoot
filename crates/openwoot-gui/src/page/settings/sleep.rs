use std::rc::Rc;

use gpui_kit::{
    App, Div, Entity, FontWeight, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        ActiveTheme as _,
        slider::{Slider, SliderState},
        v_flex,
    },
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
const LABEL_WIDTH: f32 = 44.0;
const TRACK_CENTRE: f32 = 12.0;
const TICK: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Sleep {
    pub(super) dim: u8,
    pub(super) off: u8,
}

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

pub(super) fn slider_state() -> SliderState {
    SliderState::new()
        .min(0.0)
        .max(f32::from(NEVER))
        .step(1.0)
        .default_value(f32::from(NEVER))
}

pub(super) fn stop_of(value: f32) -> u8 {
    (0..=NEVER)
        .min_by(|left, right| {
            (value - f32::from(*left))
                .abs()
                .total_cmp(&(value - f32::from(*right)).abs())
        })
        .unwrap_or(NEVER)
}

pub(super) fn position(stop: u8) -> f32 {
    f32::from(stop.min(NEVER))
}

pub(super) fn stepped(
    id: &'static str,
    state: &Entity<SliderState>,
    current: u8,
    disabled: bool,
    on_pick: impl Fn(&u8, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_pick = Rc::new(on_pick);
    let last = f32::from(NEVER);
    let ticks = (0..=NEVER).filter(|stop| *stop != current).map(|stop| {
        div()
            .absolute()
            .top(px(TRACK_CENTRE - TICK / 2.0))
            .left(relative(f32::from(stop) / last))
            .ml(px(-TICK / 2.0))
            .size(px(TICK))
            .rounded_full()
            .bg(if stop < current {
                theme.background
            } else {
                theme.muted_foreground
            })
    });
    let labels = (0..=NEVER).map(|stop| {
        let chosen = stop == current;
        let on_pick = on_pick.clone();
        let foreground = theme.foreground;
        div()
            .id((id, usize::from(stop)))
            .absolute()
            .top_0()
            .w(px(LABEL_WIDTH))
            .flex()
            .map(|this| match stop {
                0 => this.left_0().justify_start(),
                NEVER => this.right_0().justify_end(),
                _ => this
                    .left(relative(f32::from(stop) / last))
                    .ml(px(-LABEL_WIDTH / 2.0))
                    .justify_center(),
            })
            .text_sm()
            .text_color(if chosen {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .when(chosen, |this| this.font_weight(FontWeight::SEMIBOLD))
            .when(!disabled && !chosen, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.text_color(foreground))
                    .on_click(move |_, window, cx| on_pick(&stop, window, cx))
            })
            .child(label(stop))
    });
    v_flex()
        .w_full()
        .gap_1()
        .child(
            div()
                .relative()
                .w_full()
                .child(
                    Slider::new(state)
                        .disabled(disabled)
                        .text_color(theme.background),
                )
                .children(ticks),
        )
        .child(div().relative().w_full().h_5().children(labels))
        .child(
            div()
                .w_full()
                .pt_1()
                .text_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("Minutes since last key press"),
        )
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
    fn slider_values_round_to_a_stop() {
        assert_eq!(stop_of(2.4), 2);
        assert_eq!(stop_of(6.6), 7);
        assert_eq!(stop_of(-3.0), 0);
        assert_eq!(stop_of(12.0), 7);
    }

    #[test]
    fn labels_name_minutes_and_never() {
        assert_eq!(label(0), "1’");
        assert_eq!(label(6), "60’");
        assert_eq!(label(7), "Never");
    }
}
