use super::RapidTrigger;
use crate::lighting::TRAVEL_FULL;

const PRESS_HYSTERESIS: u16 = 96;
const RELEASE_FLOOR: u16 = 16;
const FOLLOW_STEP: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Fixed,
    Rapid(RapidTrigger),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Entered,
    Left,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actuation {
    pressed: bool,
    changed: bool,
    engaged: bool,
    reference: u16,
}

impl Actuation {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pressed: false,
            changed: false,
            engaged: false,
            reference: 0,
        }
    }

    #[must_use]
    pub const fn from_parts(pressed: bool, engaged: bool, reference: u16) -> Self {
        Self {
            pressed,
            changed: false,
            engaged,
            reference,
        }
    }

    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    #[must_use]
    pub const fn is_changed(&self) -> bool {
        self.changed
    }

    #[must_use]
    pub const fn is_engaged(&self) -> bool {
        self.engaged
    }

    #[must_use]
    pub const fn reference(&self) -> u16 {
        self.reference
    }

    pub const fn acknowledge(&mut self) {
        self.changed = false;
    }

    pub const fn set(&mut self, pressed: bool) {
        self.pressed = pressed;
        self.changed = true;
    }

    pub const fn release_for_rebinding(&mut self) {
        if self.pressed || self.changed {
            self.pressed = false;
            self.changed = true;
        }
    }

    pub const fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn update(&mut self, travel: u16, threshold: u16, trigger: Trigger) -> Option<Zone> {
        match trigger {
            Trigger::Fixed => {
                self.engaged = false;
                self.update_fixed(travel, threshold);
                None
            }
            Trigger::Rapid(rapid_trigger) => self.update_rapid(travel, threshold, rapid_trigger),
        }
    }

    fn update_fixed(&mut self, travel: u16, threshold: u16) {
        let mut point = i32::from(threshold);
        if self.pressed {
            let deepest = self.reference.max(travel);
            self.reference = deepest;
            if let Some(hysteresis) = remaining_hysteresis(deepest.wrapping_sub(threshold)) {
                point -= hysteresis;
            }
            if travel < release_point(point) {
                self.reference = travel;
                self.set(false);
            }
            return;
        }
        if travel < self.reference {
            self.reference = travel;
        }
        if self.reference != 0 {
            if let Some(hysteresis) = remaining_hysteresis(threshold.wrapping_sub(self.reference)) {
                point += hysteresis;
            }
            point = point.min(i32::from(TRAVEL_FULL));
        }
        if i32::from(travel) < point {
            return;
        }
        self.reference = travel;
        self.set(true);
    }

    fn update_rapid(
        &mut self,
        travel: u16,
        threshold: u16,
        rapid_trigger: RapidTrigger,
    ) -> Option<Zone> {
        let left_zone = if rapid_trigger.continuous {
            travel == 0
        } else {
            let exit =
                (i32::from(threshold) - i32::from(PRESS_HYSTERESIS)).max(i32::from(RELEASE_FLOOR));
            i32::from(travel) < exit
        };
        if left_zone {
            let zone = self.engaged.then_some(Zone::Left);
            self.engaged = false;
            if self.pressed {
                self.set(false);
            }
            return zone;
        }
        if !self.engaged {
            if self.pressed {
                self.reference = travel;
                self.engaged = true;
                return None;
            }
            if travel >= threshold {
                self.reference = travel;
                self.engaged = true;
                self.set(true);
                return Some(Zone::Entered);
            }
            return None;
        }
        let moved_up = i32::from(self.reference.wrapping_sub(travel).cast_signed());
        if !self.pressed {
            if moved_up >= -i32::from(rapid_trigger.press) && travel != TRAVEL_FULL {
                if moved_up > FOLLOW_STEP {
                    self.reference = travel;
                }
                return None;
            }
            self.reference = travel;
            self.set(true);
            return None;
        }
        if moved_up > i32::from(rapid_trigger.release) {
            self.reference = travel;
            self.set(false);
            return None;
        }
        if moved_up < -FOLLOW_STEP {
            self.reference = travel;
        }
        None
    }
}

fn remaining_hysteresis(distance: u16) -> Option<i32> {
    let past = i32::from(distance.cast_signed()).max(0);
    let remaining = i32::from(PRESS_HYSTERESIS) - past;
    (remaining >= 0).then_some(remaining)
}

fn release_point(point: i32) -> u16 {
    let floored = point.max(i32::from(RELEASE_FLOOR));
    u16::try_from(floored).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    const RAPID: RapidTrigger = RapidTrigger {
        press: 200,
        release: 300,
        continuous: false,
    };

    fn run(
        actuation: &mut Actuation,
        threshold: u16,
        trigger: Trigger,
        travels: &[u16],
    ) -> Vec<bool> {
        travels
            .iter()
            .map(|&travel| {
                actuation.update(travel, threshold, trigger);
                actuation.acknowledge();
                actuation.is_pressed()
            })
            .collect()
    }

    #[test]
    fn a_fixed_point_presses_at_the_threshold_and_releases_below_it_with_shrinking_hysteresis() {
        let mut actuation = Actuation::new();
        assert_eq!(
            run(
                &mut actuation,
                1000,
                Trigger::Fixed,
                &[500, 999, 1000, 1050, 955, 953]
            ),
            [false, false, true, true, true, false]
        );
        let mut deep = Actuation::new();
        assert_eq!(
            run(&mut deep, 1000, Trigger::Fixed, &[1200, 1001, 999]),
            [true, true, false]
        );
    }

    #[test]
    fn a_released_key_needs_the_hysteresis_again_after_a_partial_lift() {
        let mut actuation = Actuation::new();
        run(&mut actuation, 1000, Trigger::Fixed, &[1100, 800]);
        assert!(!actuation.is_pressed());
        assert_eq!(
            run(&mut actuation, 1000, Trigger::Fixed, &[950, 1000, 1001]),
            [false, true, true]
        );
        let mut near = Actuation::new();
        run(&mut near, 1000, Trigger::Fixed, &[1100, 950]);
        assert_eq!(
            run(&mut near, 1000, Trigger::Fixed, &[1000, 1045, 1046]),
            [false, false, true]
        );
    }

    #[test]
    fn rapid_trigger_follows_direction_changes_inside_the_zone() {
        let mut actuation = Actuation::new();
        let trigger = Trigger::Rapid(RAPID);
        assert_eq!(actuation.update(1000, 1000, trigger), Some(Zone::Entered));
        actuation.acknowledge();
        assert_eq!(
            run(
                &mut actuation,
                1000,
                trigger,
                &[2000, 1701, 1699, 1800, 1899, 1900]
            ),
            [true, true, false, false, false, true]
        );
        assert_eq!(actuation.update(905, 1000, trigger), None);
        assert_eq!(actuation.update(900, 1000, trigger), Some(Zone::Left));
        assert!(!actuation.is_pressed() && !actuation.engaged);
    }

    #[test]
    fn continuous_rapid_trigger_stays_engaged_until_the_key_is_at_rest() {
        let mut actuation = Actuation::new();
        let trigger = Trigger::Rapid(RapidTrigger {
            continuous: true,
            ..RAPID
        });
        assert_eq!(
            run(&mut actuation, 1000, trigger, &[1000, 600, 100, 301, 1]),
            [true, false, false, true, true]
        );
        assert_eq!(actuation.update(0, 1000, trigger), Some(Zone::Left));
        assert!(!actuation.is_pressed());
    }

    #[test]
    fn rapid_trigger_presses_at_full_travel_even_inside_the_sensitivity() {
        let mut actuation = Actuation::new();
        let trigger = Trigger::Rapid(RAPID);
        run(&mut actuation, 1000, trigger, &[4000, 3600]);
        assert!(!actuation.is_pressed());
        assert_eq!(run(&mut actuation, 1000, trigger, &[4095]), [true]);
    }

    #[test]
    fn rapid_trigger_measures_the_next_press_from_the_release_point() {
        let mut actuation = Actuation::new();
        let trigger = Trigger::Rapid(RAPID);
        run(&mut actuation, 1000, trigger, &[1000, 2000]);
        assert_eq!(
            run(&mut actuation, 1000, trigger, &[1690, 1750, 1890, 1891]),
            [false, false, false, true]
        );
    }

    #[test]
    fn a_key_held_into_rapid_trigger_releases_on_the_upstroke() {
        let mut actuation = Actuation::new();
        run(&mut actuation, 1000, Trigger::Fixed, &[3000]);
        let trigger = Trigger::Rapid(RapidTrigger {
            continuous: true,
            ..RAPID
        });
        assert_eq!(
            run(&mut actuation, 1000, trigger, &[2900, 2500, 2700, 2901]),
            [true, false, false, true]
        );
    }

    #[test]
    fn a_fixed_interlude_forgets_the_rapid_trigger_zone() {
        let trigger = Trigger::Rapid(RapidTrigger {
            continuous: true,
            ..RAPID
        });
        let mut actuation = Actuation::new();
        run(&mut actuation, 1000, trigger, &[1000]);
        run(&mut actuation, 1000, Trigger::Fixed, &[1000, 0]);
        assert_eq!(
            run(&mut actuation, 1000, trigger, &[50, 300, 999, 1000]),
            [false, false, false, true]
        );
    }
}
