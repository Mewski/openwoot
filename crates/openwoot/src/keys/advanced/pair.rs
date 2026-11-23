use openwoot_protocol::advanced::Resolution;

const FULL_TRAVEL: u16 = 0xFAD;
const SAME_DEPTH: i32 = 159;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Side {
    pub pressed: bool,
    pub travel: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Last {
    #[default]
    Neither,
    First,
    Second,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairState {
    sides: [Side; 2],
    last: Last,
    first_out: bool,
    second_out: bool,
    both: bool,
}

impl PairState {
    #[must_use]
    pub const fn sides(&self) -> [Side; 2] {
        self.sides
    }

    #[must_use]
    pub const fn last(&self) -> Last {
        self.last
    }

    #[must_use]
    pub const fn outputs(&self) -> (bool, bool) {
        (self.first_out, self.second_out)
    }

    pub(in crate::keys) fn capture(&mut self, second: bool, pressed: bool, travel: u16) -> bool {
        let (index, which) = if second {
            (1, Last::Second)
        } else {
            (0, Last::First)
        };
        if let Some(side) = self.sides.get_mut(index) {
            if !side.pressed && pressed {
                self.last = which;
            }
            *side = Side { pressed, travel };
        }
        self.has_travel()
    }

    pub(in crate::keys) fn record(&mut self, second: bool, pressed: bool, travel: u16) -> bool {
        let [first, other] = &mut self.sides;
        let side = if second { other } else { first };
        side.travel = travel;
        if !pressed {
            side.pressed = false;
        }
        self.has_travel()
    }

    pub(in crate::keys) fn resolve_depth(&mut self, both_bottomed_out: Resolution) -> (bool, bool) {
        let [first, second] = self.sides;
        let (mut on_first, mut on_second) = (first.pressed, second.pressed);
        if on_first && on_second {
            let difference = i32::from(first.travel.cast_signed()) - i32::from(second.travel);
            if (difference + SAME_DEPTH).cast_unsigned() <= (2 * SAME_DEPTH).cast_unsigned() {
                if first.travel > FULL_TRAVEL && second.travel > FULL_TRAVEL {
                    (on_first, on_second) =
                        apply(both_bottomed_out, self.last, on_first, on_second);
                } else if self.first_out || self.second_out || self.both {
                    (on_first, on_second) = (self.first_out, self.second_out);
                } else {
                    (on_first, on_second) = (self.last != Last::Second, self.last == Last::Second);
                }
            } else if first.travel <= second.travel {
                on_first = false;
            } else {
                on_second = false;
            }
        }
        self.both = first.pressed && second.pressed;
        self.first_out = on_first;
        self.second_out = on_second;
        (on_first, on_second)
    }

    pub(in crate::keys) fn resolve_socd(
        &mut self,
        resolution: Resolution,
        both_when_bottomed_out: bool,
    ) -> (bool, bool) {
        let [first, second] = self.sides;
        let (on_first, on_second) = (first.pressed, second.pressed);
        if !(on_first && on_second) {
            self.first_out = false;
            return (on_first, on_second);
        }
        if both_when_bottomed_out && first.travel > FULL_TRAVEL && second.travel > FULL_TRAVEL {
            self.first_out = true;
            return (on_first, on_second);
        }
        if self.first_out {
            if first.travel <= FULL_TRAVEL {
                self.first_out = false;
                if second.travel > FULL_TRAVEL {
                    self.last = Last::Second;
                }
            } else if !both_when_bottomed_out || second.travel <= FULL_TRAVEL {
                self.first_out = false;
                self.last = Last::First;
            }
        }
        apply(resolution, self.last, on_first, on_second)
    }

    const fn has_travel(&self) -> bool {
        let [first, second] = self.sides;
        first.travel != 0 || second.travel != 0
    }
}

const fn apply(resolution: Resolution, last: Last, first: bool, second: bool) -> (bool, bool) {
    match resolution {
        Resolution::Both => (first, second),
        Resolution::First => (first, false),
        Resolution::Second => (false, second),
        Resolution::LastPressed => match last {
            Last::First => (first, false),
            Last::Second => (false, second),
            Last::Neither => (false, false),
        },
        Resolution::Neither => (false, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn both(first: u16, second: u16) -> PairState {
        let mut state = PairState::default();
        state.capture(false, true, first);
        state.capture(true, true, second);
        state
    }

    #[test]
    fn the_deeper_key_wins_and_close_depths_keep_the_decision() {
        let mut state = both(2000, 1000);
        assert_eq!(state.resolve_depth(Resolution::Both), (true, false));
        state.capture(true, true, 1900);
        assert_eq!(state.resolve_depth(Resolution::Both), (true, false));
        state.capture(true, true, 2200);
        assert_eq!(state.resolve_depth(Resolution::Both), (false, true));
        let mut bottomed = both(4050, 4060);
        assert_eq!(
            bottomed.resolve_depth(Resolution::LastPressed),
            (false, true)
        );
    }

    #[test]
    fn socd_applies_the_rule_while_both_are_held() {
        assert_eq!(
            both(1000, 1000).resolve_socd(Resolution::LastPressed, false),
            (false, true)
        );
        assert_eq!(
            both(1000, 1000).resolve_socd(Resolution::First, false),
            (true, false)
        );
        assert_eq!(
            both(1000, 1000).resolve_socd(Resolution::Neither, false),
            (false, false)
        );
        let mut full = both(4050, 4060);
        assert_eq!(full.resolve_socd(Resolution::First, true), (true, true));
        full.capture(true, true, 3000);
        assert_eq!(
            full.resolve_socd(Resolution::LastPressed, true),
            (true, false)
        );
    }

    #[test]
    fn keys_pressed_together_at_one_depth_give_the_one_pressed_last() {
        let mut state = both(2000, 2050);
        assert_eq!(state.resolve_depth(Resolution::Both), (false, true));
        state.capture(false, true, 2100);
        assert_eq!(state.resolve_depth(Resolution::Both), (false, true));
    }

    #[test]
    fn neither_at_the_bottom_stays_neither_until_one_key_is_deeper() {
        let mut state = both(4050, 4060);
        assert_eq!(state.resolve_depth(Resolution::Neither), (false, false));
        state.capture(false, true, 3950);
        assert_eq!(state.resolve_depth(Resolution::Neither), (false, false));
        state.capture(false, true, 3800);
        assert_eq!(state.resolve_depth(Resolution::Neither), (false, true));
    }

    #[test]
    fn socd_forgets_the_bottomed_out_pair_when_one_key_is_released() {
        let mut state = both(4050, 4060);
        assert_eq!(
            state.resolve_socd(Resolution::LastPressed, true),
            (true, true)
        );
        state.capture(true, false, 0);
        assert_eq!(
            state.resolve_socd(Resolution::LastPressed, true),
            (true, false)
        );
        state.capture(true, true, 2000);
        assert_eq!(
            state.resolve_socd(Resolution::LastPressed, true),
            (false, true)
        );
    }
}
