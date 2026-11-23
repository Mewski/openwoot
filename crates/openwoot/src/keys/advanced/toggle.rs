use heapless::Vec;
use openwoot_protocol::keys::Action;

use super::{Application, Applications};
use crate::lighting::TRAVEL_FULL;

const TAP_MAX_MS: u16 = 199;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Latch {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleState {
    pressed: bool,
    armed: bool,
    latched: bool,
    hold_ms: u16,
}

impl ToggleState {
    #[must_use]
    pub const fn is_latched(&self) -> bool {
        self.latched
    }

    pub(in crate::keys) const fn input(&mut self, pressed: bool, changed: bool) -> Option<Latch> {
        self.pressed = pressed;
        if !changed {
            return None;
        }
        if pressed {
            self.hold_ms = 0;
            self.armed = true;
            return None;
        }
        let armed = self.armed;
        self.armed = false;
        if !armed {
            return None;
        }
        if self.hold_ms <= TAP_MAX_MS {
            self.latched = !self.latched;
            if self.latched {
                return Some(Latch::On);
            }
        } else {
            self.latched = false;
        }
        Some(Latch::Off)
    }

    pub(in crate::keys) fn run(
        &mut self,
        action: Action,
        travel: u16,
        elapsed_ms: u16,
    ) -> Applications {
        if self.pressed {
            self.hold_ms = self.hold_ms.saturating_add(elapsed_ms);
        }
        let mut applications = Vec::new();
        let _ = applications.push(Application {
            action,
            travel: if self.latched { TRAVEL_FULL } else { travel },
            pressed: self.pressed || self.latched,
        });
        applications
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_press_flips_the_latch_and_a_long_press_clears_it() {
        let mut state = ToggleState::default();
        assert_eq!(state.input(true, true), None);
        state.run(Action::Keyboard(4), 2000, 100);
        assert_eq!(state.input(false, true), Some(Latch::On));
        let latched = state.run(Action::Keyboard(4), 0, 10);
        assert_eq!(
            latched
                .first()
                .map(|application| (application.travel, application.pressed)),
            Some((TRAVEL_FULL, true))
        );
        state.input(true, true);
        state.run(Action::Keyboard(4), 2000, 150);
        state.run(Action::Keyboard(4), 2000, 50);
        assert_eq!(state.input(false, true), Some(Latch::Off));
        assert!(!state.is_latched());
    }

    #[test]
    fn a_hold_longer_than_the_counter_is_not_a_tap() {
        let mut state = ToggleState::default();
        state.input(true, true);
        for _ in 0..66 {
            state.run(Action::Keyboard(4), 2000, 1000);
        }
        assert_eq!(state.input(false, true), Some(Latch::Off));
        assert!(!state.is_latched());
    }

    #[test]
    fn a_release_without_its_press_does_not_latch() {
        let mut state = ToggleState::default();
        assert_eq!(state.input(false, true), None);
        assert!(!state.is_latched());
    }

    #[test]
    fn a_press_of_199_ms_is_a_tap_and_one_of_200_ms_is_a_hold() {
        let mut tapped = ToggleState::default();
        tapped.input(true, true);
        tapped.run(Action::Keyboard(4), 2000, 199);
        assert_eq!(tapped.input(false, true), Some(Latch::On));
        let mut held = ToggleState::default();
        held.input(true, true);
        held.run(Action::Keyboard(4), 2000, 200);
        assert_eq!(held.input(false, true), Some(Latch::Off));
        assert!(!held.is_latched());
    }
}
