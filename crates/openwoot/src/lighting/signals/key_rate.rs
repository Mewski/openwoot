const SLOTS: usize = 66;
const STEP_MS: u32 = 15;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyRate {
    stamp_ms: u32,
    slot: usize,
    sum: u16,
    counts: [u8; SLOTS],
}

impl KeyRate {
    pub(crate) const fn new(now_ms: u32) -> Self {
        Self {
            stamp_ms: now_ms,
            slot: 0,
            sum: 0,
            counts: [0; SLOTS],
        }
    }

    pub(crate) fn note_press(&mut self) {
        if let Some(count) = self.counts.get_mut(self.slot)
            && *count != u8::MAX
        {
            *count += 1;
            self.sum = self.sum.wrapping_add(1);
        }
    }

    pub(crate) fn step(&mut self, now_ms: u32) -> Option<f32> {
        if now_ms.wrapping_sub(self.stamp_ms) < STEP_MS {
            return None;
        }
        self.slot = (self.slot + 1) % SLOTS;
        self.stamp_ms = now_ms;
        if let Some(count) = self.counts.get_mut(self.slot) {
            self.sum = self.sum.wrapping_sub(u16::from(*count));
            *count = 0;
        }
        Some(f32::from(self.sum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_count_for_one_window_of_sixty_six_slots() {
        let mut rate = KeyRate::new(0);
        rate.note_press();
        rate.note_press();
        assert_eq!(rate.step(14), None);
        assert_eq!(rate.step(15), Some(2.0));
        rate.note_press();
        let mut now = 15;
        for _ in 0..64 {
            now += 15;
            assert_eq!(rate.step(now), Some(3.0));
        }
        now += 15;
        assert_eq!(rate.step(now), Some(1.0));
        now += 15;
        assert_eq!(rate.step(now), Some(0.0));
    }

    #[test]
    fn a_slot_counts_at_most_255_presses() {
        let mut rate = KeyRate::new(0);
        for _ in 0..300 {
            rate.note_press();
        }
        assert_eq!(rate.step(20), Some(255.0));
    }
}
