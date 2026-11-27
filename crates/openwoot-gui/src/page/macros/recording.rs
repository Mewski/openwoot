use std::time::Duration;

use openwoot_protocol::{keys::Action, macros::Step};

const CAPS_LOCK: u8 = 0x39;
const LEFT_GUI: u8 = 0xE3;
const MODIFIERS: std::ops::RangeInclusive<u8> = 0xE0..=0xE7;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Timing {
    #[default]
    Exact,
    Rounded,
    Coarse,
    Ignored,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Recording {
    events: Vec<Event>,
    held: Vec<u8>,
    modifiers: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Event {
    usage: u8,
    pressed: bool,
    at: Duration,
}

impl Timing {
    pub(super) const ALL: [Self; 4] = [Self::Exact, Self::Rounded, Self::Coarse, Self::Ignored];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Exact => "Exact delays",
            Self::Rounded => "Round to 10 ms",
            Self::Coarse => "Round to 100 ms",
            Self::Ignored => "No delays",
        }
    }

    const fn granularity_ms(self) -> Option<u128> {
        match self {
            Self::Exact => Some(1),
            Self::Rounded => Some(10),
            Self::Coarse => Some(100),
            Self::Ignored => None,
        }
    }

    fn delay(self, gap: Duration) -> Option<u16> {
        let granularity = self.granularity_ms()?;
        let rounded = (gap.as_millis() + granularity / 2) / granularity * granularity;
        (rounded > 0).then(|| u16::try_from(rounded).unwrap_or(u16::MAX))
    }
}

impl Recording {
    pub(super) fn with_modifiers(held: &[u8]) -> Self {
        Self {
            modifiers: held.to_vec(),
            ..Self::default()
        }
    }

    pub(super) fn press(&mut self, usage: u8, at: Duration) {
        if !self.held.contains(&usage) {
            self.held.push(usage);
            self.events.push(Event {
                usage,
                pressed: true,
                at,
            });
        }
    }

    pub(super) fn release(&mut self, usage: u8, at: Duration) {
        if let Some(position) = self.held.iter().position(|held| *held == usage) {
            self.held.remove(position);
            self.events.push(Event {
                usage,
                pressed: false,
                at,
            });
        }
    }

    pub(super) fn hold_modifiers(&mut self, held: &[u8], at: Duration) {
        let released: Vec<u8> = self
            .modifiers
            .iter()
            .copied()
            .filter(|usage| !held.contains(usage))
            .collect();
        for usage in released {
            if usage == LEFT_GUI {
                self.release_keys(at);
            }
            self.release(usage, at);
        }
        for usage in held {
            self.press(*usage, at);
        }
        self.modifiers = held.to_vec();
    }

    pub(super) fn toggle_caps_lock(&mut self, at: Duration) {
        self.press(CAPS_LOCK, at);
        self.release(CAPS_LOCK, at);
    }

    pub(super) fn steps(&self, timing: Timing) -> Vec<Step> {
        let end = self.events.last().map_or(Duration::ZERO, |event| event.at);
        let still_held = self.held.iter().rev().map(|usage| Event {
            usage: *usage,
            pressed: false,
            at: end,
        });
        let mut steps = Vec::new();
        let mut previous: Option<Duration> = None;
        for event in self.events.iter().copied().chain(still_held) {
            let delay =
                previous.and_then(|previous| timing.delay(event.at.saturating_sub(previous)));
            previous = Some(event.at);
            if let Some(delay) = delay {
                steps.push(Step::Delay(delay));
            }
            let action = Action::Keyboard(event.usage);
            match steps.last_mut() {
                Some(last) if !event.pressed && delay.is_none() && *last == Step::Press(action) => {
                    *last = Step::Tap(action);
                }
                _ if event.pressed => steps.push(Step::Press(action)),
                _ => steps.push(Step::Release(action)),
            }
        }
        steps
    }

    fn release_keys(&mut self, at: Duration) {
        let keys: Vec<u8> = self
            .held
            .iter()
            .rev()
            .copied()
            .filter(|usage| !MODIFIERS.contains(usage))
            .collect();
        for usage in keys {
            self.release(usage, at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: u8 = 0xE1;
    const KEY_A: u8 = 0x04;
    const KEY_B: u8 = 0x05;
    const KEY_C: u8 = 0x06;

    fn at(milliseconds: u64) -> Duration {
        Duration::from_millis(milliseconds)
    }

    fn press(usage: u8) -> Step {
        Step::Press(Action::Keyboard(usage))
    }

    fn release(usage: u8) -> Step {
        Step::Release(Action::Keyboard(usage))
    }

    fn tap(usage: u8) -> Step {
        Step::Tap(Action::Keyboard(usage))
    }

    fn typed_a_then_b() -> Recording {
        let mut recording = Recording::default();
        recording.press(KEY_A, at(1_000));
        recording.release(KEY_A, at(1_080));
        recording.press(KEY_B, at(1_204));
        recording.release(KEY_B, at(1_271));
        recording
    }

    #[test]
    fn exact_timing_keeps_every_press_release_and_pause() {
        assert_eq!(
            typed_a_then_b().steps(Timing::Exact),
            [
                press(KEY_A),
                Step::Delay(80),
                release(KEY_A),
                Step::Delay(124),
                press(KEY_B),
                Step::Delay(67),
                release(KEY_B),
            ]
        );
    }

    #[test]
    fn rounded_timing_rounds_each_pause_to_its_step() {
        assert_eq!(
            typed_a_then_b().steps(Timing::Rounded),
            [
                press(KEY_A),
                Step::Delay(80),
                release(KEY_A),
                Step::Delay(120),
                press(KEY_B),
                Step::Delay(70),
                release(KEY_B),
            ]
        );
        assert_eq!(
            typed_a_then_b().steps(Timing::Coarse),
            [
                press(KEY_A),
                Step::Delay(100),
                release(KEY_A),
                Step::Delay(100),
                press(KEY_B),
                Step::Delay(100),
                release(KEY_B),
            ]
        );
    }

    #[test]
    fn without_delays_a_press_and_its_release_become_a_tap() {
        assert_eq!(
            typed_a_then_b().steps(Timing::Ignored),
            [tap(KEY_A), tap(KEY_B)]
        );
        let mut rolled = Recording::default();
        rolled.press(KEY_A, at(0));
        rolled.press(KEY_B, at(30));
        rolled.release(KEY_A, at(60));
        rolled.release(KEY_B, at(90));
        assert_eq!(
            rolled.steps(Timing::Ignored),
            [press(KEY_A), press(KEY_B), release(KEY_A), release(KEY_B)]
        );
    }

    #[test]
    fn a_pause_that_rounds_to_nothing_leaves_a_tap() {
        let mut quick = Recording::default();
        quick.press(KEY_A, at(0));
        quick.release(KEY_A, at(4));
        assert_eq!(quick.steps(Timing::Rounded), [tap(KEY_A)]);
        assert_eq!(
            quick.steps(Timing::Exact),
            [press(KEY_A), Step::Delay(4), release(KEY_A)]
        );
    }

    #[test]
    fn repeats_of_a_held_key_and_releases_of_unknown_keys_are_skipped() {
        let mut recording = Recording::default();
        recording.release(KEY_B, at(0));
        recording.press(KEY_A, at(10));
        recording.press(KEY_A, at(500));
        recording.press(KEY_A, at(530));
        recording.release(KEY_A, at(600));
        assert_eq!(
            recording.steps(Timing::Exact),
            [press(KEY_A), Step::Delay(590), release(KEY_A)]
        );
    }

    #[test]
    fn keys_still_held_at_the_end_are_released() {
        let mut recording = Recording::default();
        recording.hold_modifiers(&[SHIFT], at(0));
        recording.press(KEY_A, at(100));
        assert_eq!(
            recording.steps(Timing::Exact),
            [press(SHIFT), Step::Delay(100), tap(KEY_A), release(SHIFT)]
        );
    }

    #[test]
    fn modifiers_follow_the_held_flags() {
        let mut recording = Recording::default();
        recording.hold_modifiers(&[SHIFT], at(0));
        recording.press(KEY_A, at(40));
        recording.release(KEY_A, at(90));
        recording.hold_modifiers(&[], at(150));
        assert_eq!(
            recording.steps(Timing::Ignored),
            [press(SHIFT), tap(KEY_A), release(SHIFT)]
        );
    }

    #[test]
    fn letting_go_of_command_releases_the_keys_it_held() {
        let mut recording = Recording::default();
        recording.hold_modifiers(&[LEFT_GUI], at(0));
        recording.press(KEY_C, at(50));
        recording.hold_modifiers(&[], at(200));
        recording.release(KEY_C, at(210));
        assert_eq!(
            recording.steps(Timing::Exact),
            [
                press(LEFT_GUI),
                Step::Delay(50),
                press(KEY_C),
                Step::Delay(150),
                release(KEY_C),
                release(LEFT_GUI),
            ]
        );
    }

    #[test]
    fn a_caps_lock_change_is_a_tap() {
        let mut recording = Recording::default();
        recording.toggle_caps_lock(at(0));
        assert_eq!(recording.steps(Timing::Exact), [tap(CAPS_LOCK)]);
        assert_eq!(
            Recording::default().steps(Timing::Exact),
            Vec::<Step>::new()
        );
    }

    #[test]
    fn long_pauses_are_capped_at_the_longest_delay() {
        let mut recording = Recording::default();
        recording.press(KEY_A, at(0));
        recording.release(KEY_A, at(100_000));
        assert_eq!(
            recording.steps(Timing::Exact),
            [press(KEY_A), Step::Delay(u16::MAX), release(KEY_A)]
        );
    }

    #[test]
    fn modifiers_held_before_recording_are_not_recorded() {
        let mut recording = Recording::with_modifiers(&[SHIFT]);
        recording.press(KEY_A, at(10));
        recording.release(KEY_A, at(20));
        recording.hold_modifiers(&[], at(30));
        recording.hold_modifiers(&[SHIFT], at(40));
        recording.press(KEY_B, at(50));
        recording.release(KEY_B, at(60));
        recording.hold_modifiers(&[], at(70));
        assert_eq!(
            recording.steps(Timing::Ignored),
            [tap(KEY_A), press(SHIFT), tap(KEY_B), release(SHIFT)]
        );
    }
}
