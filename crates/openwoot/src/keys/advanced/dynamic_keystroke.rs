use heapless::Vec;
use openwoot_protocol::advanced::{STROKES, Stroke, StrokeEvent};

use super::{Application, Applications};
use crate::keys::follows_travel_action;

const DEFAULT_BOTTOM_OUT: u16 = 3693;
const BOTTOM_GAP: u16 = 112;
const RELEASE_HYSTERESIS: u16 = 80;
const RELEASE_FLOOR: u16 = 1;
const TIMER_NOT_STARTED: u8 = 0x3F;
const TIMER_MASK: u8 = 0x3F;
const NONE: u8 = 0;
const PRESS: u8 = 1;
const RELEASE: u8 = 2;
const TAP: u8 = 3;
const LAST_STEP: u8 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Pressed,
    Bottomed,
    Lifted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point {
    Press,
    BottomOut,
    Lift,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Queue {
    events: u8,
    pending: u8,
    timer: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DynamicKeystrokeState {
    phase: Phase,
    queues: [Queue; STROKES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Thresholds {
    pub actuation: u16,
    pub bottom_out: u16,
    pub release: u16,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            events: NONE,
            pending: 0,
            timer: 0,
        }
    }
}

impl Queue {
    fn push(&mut self, event: u8) {
        let last = if self.pending != 0 {
            if self.pending == LAST_STEP {
                return;
            }
            self.pending
        } else if self.events & 3 != NONE {
            0
        } else {
            self.events |= event;
            self.pending = 0;
            if self.events & 3 != NONE {
                self.timer = TIMER_NOT_STARTED;
            }
            return;
        };
        let events = self.events | RELEASE << (2 * last);
        if event == RELEASE {
            self.events = events;
        } else {
            self.events = events | event << (2 * (last + 1));
            self.pending = (last + 1) & 3;
        }
    }

    fn elapse(&mut self, elapsed_ms: u16, tap_ms: u8) -> bool {
        if self.timer == TIMER_NOT_STARTED {
            self.timer = tap_ms & TIMER_MASK;
            return false;
        }
        if elapsed_ms < u16::from(self.timer) {
            self.timer = (self.timer - u8::try_from(elapsed_ms).unwrap_or_default()) & TIMER_MASK;
            return false;
        }
        self.timer = 0;
        true
    }

    fn play(&mut self, elapsed_ms: u16, tap_ms: u8) -> bool {
        match self.events & 3 {
            PRESS => {
                self.elapse(elapsed_ms, tap_ms);
                true
            }
            RELEASE => {
                if self.elapse(elapsed_ms, tap_ms) {
                    let pending = self.pending.saturating_sub(1);
                    self.events >>= 2;
                    self.pending = pending;
                    if self.events & 3 != NONE {
                        self.timer = TIMER_NOT_STARTED;
                    }
                }
                false
            }
            TAP => {
                if self.elapse(elapsed_ms, tap_ms) {
                    self.events &= !1;
                    self.timer = TIMER_NOT_STARTED;
                }
                true
            }
            _ => false,
        }
    }
}

impl Thresholds {
    #[must_use]
    pub(crate) fn new(actuation: u16, bottom_out: Option<u16>, travel_limit: u16) -> Self {
        let bottom = bottom_out.unwrap_or(DEFAULT_BOTTOM_OUT).min(travel_limit);
        let gap = actuation.wrapping_add(BOTTOM_GAP);
        let bottom_out = if i32::from(actuation) + i32::from(BOTTOM_GAP) < i32::from(bottom) {
            bottom
        } else {
            gap
        };
        let release = if actuation.cast_signed() <= (RELEASE_HYSTERESIS + 1).cast_signed() {
            RELEASE_FLOOR
        } else {
            actuation - RELEASE_HYSTERESIS
        };
        Self {
            actuation,
            bottom_out,
            release,
        }
    }
}

impl DynamicKeystrokeState {
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.queues.iter().any(|queue| queue.events != NONE)
    }

    pub(in crate::keys) fn input(
        &mut self,
        strokes: &[Stroke; STROKES],
        travel: u16,
        thresholds: Thresholds,
    ) -> (Option<bool>, bool) {
        let mut edge = None;
        match self.phase {
            Phase::Idle => {
                if travel >= thresholds.actuation {
                    self.phase = Phase::Pressed;
                    self.queue(strokes, Point::Press);
                    edge = Some(true);
                }
            }
            Phase::Pressed | Phase::Lifted => {
                if travel >= thresholds.bottom_out {
                    self.phase = Phase::Bottomed;
                    self.queue(strokes, Point::BottomOut);
                } else if travel < thresholds.release {
                    self.phase = Phase::Idle;
                    self.queue(strokes, Point::Release);
                    edge = Some(false);
                }
            }
            Phase::Bottomed => {
                let lift =
                    i32::from(thresholds.bottom_out.cast_signed()) - i32::from(RELEASE_HYSTERESIS);
                if i32::from(travel) < lift {
                    self.phase = Phase::Lifted;
                    self.queue(strokes, Point::Lift);
                }
            }
        }
        (edge, self.is_playing() || self.phase != Phase::Idle)
    }

    pub(in crate::keys) fn run(
        &mut self,
        strokes: &[Stroke; STROKES],
        travel: u16,
        elapsed_ms: u16,
        tap_ms: u8,
    ) -> (Applications, bool) {
        let mut applications = Vec::new();
        let mut active = false;
        for (queue, stroke) in self.queues.iter_mut().zip(strokes) {
            if stroke.action == openwoot_protocol::keys::Action::None {
                continue;
            }
            let pressed = follows_travel_action(stroke.action) || queue.play(elapsed_ms, tap_ms);
            if pressed {
                let _ = applications.push(Application {
                    action: stroke.action,
                    travel,
                    pressed: true,
                });
            }
            if queue.events != NONE {
                active = true;
            }
        }
        (applications, active || self.phase != Phase::Idle)
    }

    fn queue(&mut self, strokes: &[Stroke; STROKES], point: Point) {
        for (queue, stroke) in self.queues.iter_mut().zip(strokes) {
            let event = match point {
                Point::Press => stroke.on_press,
                Point::BottomOut => stroke.on_bottom_out,
                Point::Lift => stroke.on_lift,
                Point::Release => stroke.on_release,
            };
            match event {
                StrokeEvent::Nothing => {}
                StrokeEvent::Press => queue.push(PRESS),
                StrokeEvent::Release => queue.push(RELEASE),
                StrokeEvent::Tap => queue.push(TAP),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use openwoot_protocol::keys::Action;

    use super::*;

    fn tap_and_press(action: Action) -> [Stroke; STROKES] {
        let mut strokes = [Stroke::default(); STROKES];
        if let Some(first) = strokes.first_mut() {
            *first = Stroke {
                action,
                on_press: StrokeEvent::Tap,
                on_bottom_out: StrokeEvent::Press,
                on_lift: StrokeEvent::Release,
                on_release: StrokeEvent::Nothing,
            };
        }
        strokes
    }

    fn pressed(state: &mut DynamicKeystrokeState, strokes: &[Stroke; STROKES]) -> bool {
        let (applications, _) = state.run(strokes, 1000, 5, 20);
        !applications.is_empty()
    }

    #[test]
    fn the_thresholds_keep_a_gap_to_the_bottom_and_a_release_hysteresis() {
        assert_eq!(
            Thresholds::new(1000, None, 4000),
            Thresholds {
                actuation: 1000,
                bottom_out: 3693,
                release: 920
            }
        );
        assert_eq!(Thresholds::new(3650, Some(3000), 4000).bottom_out, 3762);
        assert_eq!(Thresholds::new(60, Some(500), 400).release, 1);
        assert_eq!(Thresholds::new(60, Some(500), 400).bottom_out, 400);
    }

    #[test]
    fn a_tap_at_the_press_point_holds_for_the_tap_length_then_gives_a_gap() {
        let strokes = tap_and_press(Action::Keyboard(0x04));
        let mut state = DynamicKeystrokeState::default();
        let thresholds = Thresholds::new(1000, None, 4000);
        assert_eq!(state.input(&strokes, 1200, thresholds), (Some(true), true));
        let outputs: std::vec::Vec<bool> = (0..6).map(|_| pressed(&mut state, &strokes)).collect();
        assert_eq!(outputs, [true, true, true, true, true, false]);
    }

    #[test]
    fn an_event_behind_a_press_turns_it_into_a_tap() {
        let strokes = tap_and_press(Action::Keyboard(0x04));
        let mut state = DynamicKeystrokeState::default();
        let thresholds = Thresholds::new(1000, None, 4000);
        state.input(&strokes, 1200, thresholds);
        state.input(&strokes, 3800, thresholds);
        assert_eq!(state.phase(), Phase::Bottomed);
        state.input(&strokes, 3000, thresholds);
        assert_eq!(state.phase(), Phase::Lifted);
        assert_eq!(state.input(&strokes, 0, thresholds), (Some(false), true));
        let mut presses = 0;
        while state.is_playing() {
            if pressed(&mut state, &strokes) {
                presses += 1;
            }
        }
        assert!(presses > 0);
        assert_eq!(state.phase(), Phase::Idle);
    }
}
