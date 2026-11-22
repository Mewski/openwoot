pub const BRIGHTNESS_STEP: u8 = 5;
pub const BRIGHTNESS_MAX_FAST_SCAN: u8 = 179;
pub const BRIGHTNESS_SAVE_DELAY_MS: u32 = 2000;

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
    }

    pub(super) fn step(&mut self, increase: bool, fast_scan: bool, now_ms: u32) -> u8 {
        self.level = if increase {
            let max = if fast_scan {
                BRIGHTNESS_MAX_FAST_SCAN
            } else {
                u8::MAX
            };
            self.level.saturating_add(BRIGHTNESS_STEP).min(max)
        } else {
            self.level.saturating_sub(BRIGHTNESS_STEP)
        };
        self.save_due_ms = Some(now_ms.wrapping_add(BRIGHTNESS_SAVE_DELAY_MS));
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
