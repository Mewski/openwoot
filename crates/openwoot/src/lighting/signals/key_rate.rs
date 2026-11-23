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
        let steps = now_ms.wrapping_sub(self.stamp_ms) / STEP_MS;
        if steps == 0 {
            return None;
        }
        self.stamp_ms = self.stamp_ms.wrapping_add(steps * STEP_MS);
        match usize::try_from(steps) {
            Ok(steps) if steps < SLOTS => {
                for _ in 0..steps {
                    self.advance();
                }
            }
            _ => {
                self.counts = [0; SLOTS];
                self.sum = 0;
            }
        }
        Some(f32::from(self.sum))
    }

    fn advance(&mut self) {
        self.slot = (self.slot + 1) % SLOTS;
        if let Some(count) = self.counts.get_mut(self.slot) {
            self.sum = self.sum.wrapping_sub(u16::from(*count));
            *count = 0;
        }
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

    #[test]
    fn the_window_is_990_ms_long_whenever_the_steps_are_taken() {
        let mut rate = KeyRate::new(0);
        rate.note_press();
        let mut left_at_ms = None;
        for now_ms in (4..2000).step_by(4) {
            if rate.step(now_ms) == Some(0.0) && left_at_ms.is_none() {
                left_at_ms = Some(now_ms);
            }
        }
        assert_eq!(left_at_ms, Some(992));
    }

    #[test]
    fn a_late_step_moves_the_window_by_the_time_that_passed() {
        let mut rate = KeyRate::new(0);
        rate.note_press();
        assert_eq!(rate.step(15 * 65), Some(1.0));
        rate.note_press();
        assert_eq!(rate.step(15 * 66), Some(1.0));
        assert_eq!(rate.step(15 * 66 + 14), None);
        assert_eq!(rate.step(15 * 131), Some(0.0));

        let mut stalled = KeyRate::new(0);
        stalled.note_press();
        assert_eq!(stalled.step(5000), Some(0.0));
        stalled.note_press();
        assert_eq!(stalled.step(5010), Some(1.0));
        assert_eq!(stalled.step(5024), None);
        assert_eq!(stalled.step(5025), Some(1.0));
    }
}
