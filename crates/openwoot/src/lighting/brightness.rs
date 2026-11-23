const STEP: u8 = 5;
const MAX_FAST_SCAN: u8 = 179;
const SAVE_DELAY_MS: u32 = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Brightness {
    level: u8,
    applied: u8,
    save_due_ms: Option<u32>,
}

impl Brightness {
    pub(super) const fn new() -> Self {
        Self {
            level: u8::MAX,
            applied: 0,
            save_due_ms: None,
        }
    }

    pub(super) const fn level(&self) -> u8 {
        self.level
    }

    pub(super) const fn applied(&self) -> u8 {
        self.applied
    }

    pub(super) fn percent(&self) -> f32 {
        f32::from(self.level) * 100.0 / 255.0
    }

    pub(super) const fn set(&mut self, level: u8) {
        self.level = level;
        self.save_due_ms = None;
    }

    pub(super) fn step(&mut self, increase: bool, fast_scan: bool, now_ms: u32) -> u8 {
        self.level = if increase {
            let max = if fast_scan { MAX_FAST_SCAN } else { u8::MAX };
            self.level.saturating_add(STEP).min(max.max(self.level))
        } else {
            self.level.saturating_sub(STEP)
        };
        self.save_due_ms = Some(now_ms.wrapping_add(SAVE_DELAY_MS));
        self.level
    }

    pub(super) const fn apply(&mut self) -> bool {
        if self.level == self.applied {
            return false;
        }
        self.applied = self.level;
        true
    }

    pub(super) fn take_save(&mut self, now_ms: u32) -> Option<u8> {
        let due = self.save_due_ms?;
        if now_ms.wrapping_sub(due).cast_signed() < 0 {
            return None;
        }
        self.save_due_ms = None;
        Some(self.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_up_in_fast_scan_stops_at_the_cap_and_never_lowers_a_higher_level() {
        let mut brightness = Brightness::new();
        brightness.set(170);
        assert_eq!(brightness.step(true, true, 0), 175);
        assert_eq!(brightness.step(true, true, 0), MAX_FAST_SCAN);
        assert_eq!(brightness.step(true, true, 0), MAX_FAST_SCAN);
        brightness.set(u8::MAX);
        assert_eq!(brightness.step(true, true, 0), u8::MAX);
        assert_eq!(brightness.step(false, true, 0), 250);
    }

    #[test]
    fn a_settings_change_cancels_the_pending_save_of_a_brightness_step() {
        let mut brightness = Brightness::new();
        brightness.set(100);
        brightness.step(true, false, 0);
        brightness.set(30);
        assert_eq!(brightness.take_save(SAVE_DELAY_MS), None);
        brightness.step(false, false, 10);
        assert_eq!(brightness.take_save(10 + SAVE_DELAY_MS), Some(25));
    }
}
