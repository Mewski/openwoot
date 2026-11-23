use openwoot_protocol::lighting::Idle;

use super::overlay::{FADE_FULL, Overlay, Span};

pub const FADE_DIMMED: u8 = 127;

const DIM_FADE_MS: u16 = 2000;
const OFF_FADE_MS: u16 = 3000;
const WAKE_FADE_MS: u16 = 500;
const MILLISECONDS_PER_MINUTE: u32 = 60_000;
const MINUTES_MIN: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timer {
    config: Option<Idle>,
    stamp_ms: u32,
    elapsed_ms: u32,
    dimmed: bool,
    off: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Step {
    pub(super) fade: Option<Fade>,
    pub(super) off: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Fade {
    pub(super) span: Span,
    pub(super) overlay: Overlay,
}

impl Timer {
    pub(super) const fn new(now_ms: u32) -> Self {
        Self {
            config: None,
            stamp_ms: now_ms,
            elapsed_ms: 0,
            dimmed: false,
            off: false,
        }
    }

    pub(super) const fn set_config(&mut self, config: Option<Idle>) {
        self.config = config;
    }

    pub(super) const fn is_off(&self) -> bool {
        self.off
    }

    pub(super) const fn level(&self) -> u8 {
        if self.off {
            0
        } else if self.dimmed {
            FADE_DIMMED
        } else {
            FADE_FULL
        }
    }

    pub(super) const fn restart(&mut self, now_ms: u32) {
        self.stamp_ms = now_ms;
        self.elapsed_ms = 0;
        self.dimmed = false;
        self.off = false;
    }

    pub(super) fn advance(&mut self, now_ms: u32, level: u8) -> Step {
        let previous_ms = self.stamp_ms;
        self.stamp_ms = now_ms;
        let Some(idle) = self.config else {
            return Step::default();
        };
        self.elapsed_ms = self
            .elapsed_ms
            .saturating_add(now_ms.wrapping_sub(previous_ms));
        let minutes = self.elapsed_ms / MILLISECONDS_PER_MINUTE;
        let off_after = u32::from(idle.off_after_minutes).max(MINUTES_MIN);
        let dim_after = u32::from(idle.dim_after_minutes).max(MINUTES_MIN);
        if idle.off_enabled && minutes >= off_after {
            let fade = (!self.off).then(|| Fade::forever(level, 0, OFF_FADE_MS));
            self.off = true;
            return Step { fade, off: true };
        }
        if dim_after <= minutes && !self.dimmed {
            self.dimmed = true;
            return Step {
                fade: Some(Fade::forever(level, FADE_DIMMED, DIM_FADE_MS)),
                off: false,
            };
        }
        Step::default()
    }

    pub(super) fn reset(&mut self, level: u8) -> Option<Fade> {
        let fade = (self.dimmed || self.off).then_some(Fade {
            span: Span::Milliseconds(WAKE_FADE_MS),
            overlay: Overlay::fade(level, FADE_FULL, WAKE_FADE_MS),
        });
        self.elapsed_ms = 0;
        self.dimmed = false;
        self.off = false;
        fade
    }
}

impl Fade {
    const fn forever(from: u8, to: u8, duration_ms: u16) -> Self {
        Self {
            span: Span::Forever,
            overlay: Overlay::fade(from, to, duration_ms),
        }
    }
}
