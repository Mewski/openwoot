use heapless::Vec;
use openwoot_protocol::{
    advanced::Interrupt,
    keys::{Action, Layer},
};

use super::{Application, Applications};

const CAPS_LOCK: u8 = 0x39;

const TIMER_IDLE: u16 = u16::MAX;
const CAPS_LOCK_TAP_MS: u16 = 100;
const MODIFIERS: core::ops::RangeInclusive<u8> = 0xE0..=0xE7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Decision {
    #[default]
    Idle,
    Undecided,
    Hold,
    Tap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotChange {
    Keep,
    Release(u8),
    Take,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModTap {
    pub tap: Action,
    pub hold: Action,
    pub hold_ms: u16,
    pub interrupt: Interrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModTapState {
    decision: Decision,
    timer_ms: u16,
    slot: Option<u8>,
}

impl Default for ModTapState {
    fn default() -> Self {
        Self {
            decision: Decision::Idle,
            timer_ms: 0,
            slot: None,
        }
    }
}

impl ModTap {
    #[must_use]
    pub fn waits_for_other_keys(&self) -> bool {
        match self.interrupt {
            Interrupt::Ignore => false,
            Interrupt::Hold | Interrupt::Tap => true,
            Interrupt::Auto => match self.hold {
                Action::Keyboard(usage) => MODIFIERS.contains(&usage),
                Action::Layer(layer) => layer != Layer::Base,
                _ => false,
            },
        }
    }

    fn tap_ms(&self, tap_ms: u8) -> u16 {
        if self.tap == Action::Keyboard(CAPS_LOCK) {
            CAPS_LOCK_TAP_MS
        } else {
            u16::from(tap_ms)
        }
    }
}

impl ModTapState {
    #[must_use]
    pub const fn decision(&self) -> Decision {
        self.decision
    }

    pub(in crate::keys) const fn set_slot(&mut self, slot: Option<u8>) {
        self.slot = slot;
    }

    pub(in crate::keys) fn input(
        &mut self,
        mod_tap: &ModTap,
        pressed: bool,
        changed: bool,
    ) -> (SlotChange, bool) {
        let mut slot = SlotChange::Keep;
        if changed {
            if !pressed {
                if let Some(index) = self.slot.take() {
                    slot = SlotChange::Release(index);
                }
            } else if mod_tap.waits_for_other_keys() {
                slot = SlotChange::Take;
            }
            match self.decision {
                Decision::Idle => {
                    if pressed {
                        self.decision = Decision::Undecided;
                        self.timer_ms = TIMER_IDLE;
                    }
                }
                Decision::Undecided => {
                    if self.timer_ms >= mod_tap.hold_ms {
                        self.decision = Decision::Hold;
                        self.timer_ms = 0;
                    } else if !pressed {
                        self.decision = Decision::Tap;
                        self.timer_ms = TIMER_IDLE;
                    }
                }
                Decision::Hold => {
                    if !pressed {
                        self.decision = Decision::Idle;
                        self.timer_ms = 0;
                    }
                }
                Decision::Tap => {}
            }
        }
        (slot, self.decision != Decision::Idle)
    }

    pub(in crate::keys) fn interrupt(&mut self, mod_tap: &ModTap) -> Option<Action> {
        if self.decision != Decision::Undecided {
            return None;
        }
        let (decision, action) = if mod_tap.interrupt == Interrupt::Tap {
            (Decision::Tap, mod_tap.tap)
        } else {
            (Decision::Hold, mod_tap.hold)
        };
        self.decision = decision;
        self.timer_ms = 0;
        Some(action)
    }

    pub(in crate::keys) fn run(
        &mut self,
        mod_tap: &ModTap,
        travel: u16,
        elapsed_ms: u16,
        tap_ms: u8,
    ) -> (Applications, bool) {
        let mut applications = Vec::new();
        let press = |action| Application {
            action,
            travel,
            pressed: true,
        };
        match self.decision {
            Decision::Undecided => {
                if self.advance(elapsed_ms) < mod_tap.hold_ms {
                    return (applications, true);
                }
                self.decision = Decision::Hold;
                let _ = applications.push(press(mod_tap.hold));
            }
            Decision::Hold => {
                let _ = applications.push(press(mod_tap.hold));
            }
            Decision::Tap => {
                let _ = applications.push(press(mod_tap.tap));
                if self.advance(elapsed_ms) >= mod_tap.tap_ms(tap_ms) {
                    self.decision = Decision::Idle;
                    return (applications, false);
                }
            }
            Decision::Idle => {}
        }
        (applications, self.decision != Decision::Idle)
    }

    const fn advance(&mut self, elapsed_ms: u16) -> u16 {
        self.timer_ms = if self.timer_ms == TIMER_IDLE {
            0
        } else {
            self.timer_ms.wrapping_add(elapsed_ms)
        };
        self.timer_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT_OR_A: ModTap = ModTap {
        tap: Action::Keyboard(0x04),
        hold: Action::Keyboard(0xE1),
        hold_ms: 200,
        interrupt: Interrupt::Auto,
    };

    fn sent(state: &mut ModTapState, elapsed_ms: u16) -> Option<Action> {
        let (applications, _) = state.run(&SHIFT_OR_A, 4095, elapsed_ms, 40);
        applications.first().map(|application| application.action)
    }

    #[test]
    fn a_short_press_taps_and_a_long_press_holds() {
        let mut state = ModTapState::default();
        assert_eq!(
            state.input(&SHIFT_OR_A, true, true),
            (SlotChange::Take, true)
        );
        assert_eq!(sent(&mut state, 10), None);
        assert_eq!(sent(&mut state, 100), None);
        assert!(state.input(&SHIFT_OR_A, false, true).1);
        assert_eq!(state.decision(), Decision::Tap);
        assert_eq!(sent(&mut state, 10), Some(Action::Keyboard(0x04)));
        assert_eq!(sent(&mut state, 39), Some(Action::Keyboard(0x04)));
        assert_eq!(state.decision(), Decision::Tap);
        assert_eq!(sent(&mut state, 1), Some(Action::Keyboard(0x04)));
        assert_eq!(state.decision(), Decision::Idle);

        let mut held = ModTapState::default();
        held.input(&SHIFT_OR_A, true, true);
        assert_eq!(sent(&mut held, 0), None);
        assert_eq!(sent(&mut held, 199), None);
        assert_eq!(sent(&mut held, 1), Some(Action::Keyboard(0xE1)));
        assert_eq!(held.decision(), Decision::Hold);
        assert!(!held.input(&SHIFT_OR_A, false, true).1);
    }

    #[test]
    fn only_modifiers_and_layers_wait_for_other_keys_unless_the_mode_says_so() {
        assert!(SHIFT_OR_A.waits_for_other_keys());
        let letter = ModTap {
            hold: Action::Keyboard(0x05),
            ..SHIFT_OR_A
        };
        assert!(!letter.waits_for_other_keys());
        assert!(
            ModTap {
                interrupt: Interrupt::Tap,
                ..letter
            }
            .waits_for_other_keys()
        );
        assert!(
            !ModTap {
                interrupt: Interrupt::Ignore,
                ..SHIFT_OR_A
            }
            .waits_for_other_keys()
        );
    }
}
