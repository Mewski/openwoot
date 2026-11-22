pub(super) const VALUE_NONE: u16 = 0x0FFF;

const SAMPLE_INTERVAL_US: u16 = 1000;
const STROKE_TIMEOUT_US: u16 = 25_000;
const PLATEAU_HOLD_US: u16 = 14_999;
const STROKE_START_VELOCITY: f32 = 6.5;
const STROKE_REVERSAL: f32 = 0.166_666_67;
const QUIET_VELOCITY: f32 = 2.0;
const QUIET_SAMPLES: u8 = 3;
const PLATEAU_MAX_VELOCITY: f32 = 5.0;
const PLATEAU_MAX_SKEW: f32 = 2.0;
const SCORE_MAX: u16 = 2000;
const STABILITY_MAX: u16 = 3000;
const CANDIDATE_HITS: u8 = 3;
const CANDIDATE_MISSES: u8 = 3;
const MARGIN_KNEE: u32 = 350;
const HEADROOM: u16 = 240;
const PLATEAU_LIMIT_NUMERATOR: u32 = 229;
const PLATEAU_LIMIT_DENOMINATOR: u32 = 255;
const CONFIDENCE_ADOPT: u16 = 149;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BottomOut {
    reference: u16,
    estimate: u16,
    learned: bool,
    tracker: Tracker,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tracker {
    strength_configured: u8,
    has_upper: bool,
    upper: u16,
    lower_bound: u16,
    strength_effective: u8,
    candidate_base: u16,
    candidate_max: u16,
    candidate_hits: u8,
    candidate_misses: u8,
    score: u16,
    confidence: u16,
    stability: u16,
    max_plateau: u16,
    velocity: f32,
    stroke_peak_velocity: f32,
    quiet_samples: u8,
    stroke_active: bool,
    stroke_start_us: u16,
    last_sample_us: u16,
    last_sample_value: u16,
    plateau_active: bool,
    plateau_start_us: u16,
    plateau_max: u16,
    plateau_min: u16,
    plateau_mean: f32,
    plateau_samples: u16,
}

impl Tracker {
    const fn new() -> Self {
        Self {
            strength_configured: 0,
            has_upper: false,
            upper: 0,
            lower_bound: VALUE_NONE,
            strength_effective: 0,
            candidate_base: 0,
            candidate_max: 0,
            candidate_hits: 0,
            candidate_misses: 0,
            score: 0,
            confidence: 0,
            stability: 0,
            max_plateau: 0,
            velocity: 0.0,
            stroke_peak_velocity: 0.0,
            quiet_samples: 0,
            stroke_active: false,
            stroke_start_us: 0,
            last_sample_us: 0,
            last_sample_value: 0,
            plateau_active: false,
            plateau_start_us: 0,
            plateau_max: 0,
            plateau_min: 0,
            plateau_mean: 0.0,
            plateau_samples: 0,
        }
    }
}

impl Default for BottomOut {
    fn default() -> Self {
        Self::new()
    }
}

impl BottomOut {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reference: VALUE_NONE,
            estimate: VALUE_NONE,
            learned: false,
            tracker: Tracker::new(),
        }
    }

    #[must_use]
    pub const fn estimate(&self) -> Option<u16> {
        if self.estimate == VALUE_NONE {
            None
        } else {
            Some(self.estimate)
        }
    }

    #[must_use]
    pub const fn reference(&self) -> Option<u16> {
        if self.reference == VALUE_NONE {
            None
        } else {
            Some(self.reference)
        }
    }

    #[must_use]
    pub const fn is_learned(&self) -> bool {
        self.learned
    }

    #[must_use]
    pub const fn confidence(&self) -> u16 {
        self.tracker.confidence
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn note_press_edge(&mut self, previous_value: u16, now_us: u16) {
        if self.tracker.last_sample_value != 0 {
            self.tracker.last_sample_value = previous_value;
            self.tracker.last_sample_us = now_us;
            self.tracker.stroke_active = false;
        }
    }

    pub(crate) fn observe(
        &mut self,
        value: u16,
        now_us: u16,
        strength: u8,
        rest_level: u16,
        bottom_level: u16,
    ) {
        let tracker = &mut self.tracker;
        let last_value = tracker.last_sample_value;
        if last_value == 0 || tracker.strength_configured != strength {
            if last_value != 0 {
                self.reset();
            }
            let tracker = &mut self.tracker;
            tracker.strength_configured = strength;
            tracker.strength_effective = strength;
            tracker.last_sample_value = value;
            tracker.last_sample_us = now_us;
            return;
        }
        let elapsed_us = now_us.wrapping_sub(tracker.last_sample_us);
        if elapsed_us <= SAMPLE_INTERVAL_US {
            return;
        }
        tracker.last_sample_us = now_us;
        tracker.last_sample_value = value;
        let velocity =
            (f32::from(value) - f32::from(last_value)) / (f32::from(elapsed_us) / 1000.0);
        tracker.velocity = velocity;

        if !tracker.stroke_active {
            if velocity <= (u32::from(bottom_level / 22) as i32) as f32 + STROKE_START_VELOCITY {
                return;
            }
            tracker.stroke_peak_velocity = velocity;
            tracker.quiet_samples = 0;
            tracker.stroke_active = true;
            tracker.plateau_active = false;
            tracker.stroke_start_us = now_us;
            tracker.follow_stroke(value, now_us);
            return;
        }
        if !tracker.plateau_active {
            if now_us.wrapping_sub(tracker.stroke_start_us) > STROKE_TIMEOUT_US {
                tracker.stroke_active = false;
                return;
            }
            tracker.follow_stroke(value, now_us);
            return;
        }
        if !tracker.extend_plateau(value) {
            return;
        }
        if now_us.wrapping_sub(tracker.plateau_start_us) > PLATEAU_HOLD_US {
            self.accept_plateau(u32::from(strength), rest_level, bottom_level);
        }
    }

    pub(crate) fn bottom_level(&mut self, value: u16, rest_level: u16, bottom_level: u16) -> u16 {
        if self.learned {
            if self.estimate < VALUE_NONE {
                return self.estimate;
            }
            self.learned = false;
        }
        let mut level = bottom_level;
        if u32::from(value) > u32::from(level) + u32::from(HEADROOM) {
            level = value - HEADROOM;
        }
        let max_plateau = u32::from(self.tracker.max_plateau);
        if max_plateau == 0 {
            return level;
        }
        let limit = PLATEAU_LIMIT_NUMERATOR * max_plateau / PLATEAU_LIMIT_DENOMINATOR;
        let mut clamped = u32::from(level).min(limit);
        if u32::from(rest_level) >= clamped {
            clamped = u32::from(rest_level);
        }
        level = clamped as u16;
        if self.tracker.confidence > CONFIDENCE_ADOPT || self.estimate < level {
            self.learned = true;
            return self.estimate;
        }
        level
    }

    fn publish(&mut self, estimate: u32, strength_effective: u32, unused_strength: u32, gain: u32) {
        let tracker = &mut self.tracker;
        tracker.strength_effective = strength_effective as u8;
        let confidence = u32::from(
            tracker
                .score
                .wrapping_add((127 * unused_strength / 255) as u16),
        );
        tracker.confidence = confidence.min(u32::from(SCORE_MAX)) as u16;
        let mut estimate = estimate;
        if estimate > 500 {
            let scale = if estimate > 1499 {
                255 * gain
            } else {
                gain * (255 * (estimate - 500) / 1000)
            };
            estimate = u32::from(estimate.wrapping_sub(10 * (scale / 255) / 255) as u16);
        }
        self.estimate = estimate as u16;
    }

    fn accept_plateau(&mut self, strength: u32, rest_level: u16, bottom_level: u16) {
        let reference = u32::from(self.reference);
        let tracker = &mut self.tracker;
        let plateau = u32::from(tracker.plateau_min);
        let mut drop = 0;
        tracker.plateau_active = false;

        let estimate = if reference <= plateau {
            let bonus = (-12 * i32::from(tracker.strength_effective) / 255 + 20) as u16;
            tracker.score = add_capped(tracker.score, bonus, SCORE_MAX);
            if u32::from(tracker.max_plateau) < plateau {
                tracker.max_plateau = plateau as u16;
            }
            reference
        } else {
            let strength_effective = u32::from(tracker.strength_effective);
            let penalty = (90 * strength_effective / 255 + 60) as u16;
            tracker.score = tracker.score.saturating_sub(penalty);
            if u32::from(tracker.max_plateau) < plateau {
                tracker.max_plateau = plateau as u16;
            }
            if self.reference == VALUE_NONE {
                if plateau < u32::from(rest_level).midpoint(u32::from(bottom_level)) {
                    self.tracker.stroke_active = false;
                    return;
                }
            } else {
                let mut limit = u32::from((123 * reference / 125).wrapping_sub(6) as u16);
                if strength != 0 && limit >= u32::from(tracker.lower_bound) {
                    limit = u32::from(tracker.lower_bound);
                }
                if plateau < limit {
                    tracker.stroke_active = false;
                    if strength == 0 {
                        self.publish(reference, 0, 0, 0);
                    } else {
                        self.update_estimate(reference, plateau, strength);
                    }
                    return;
                }
                if strength != 0 && !tracker.has_upper {
                    tracker.upper = reference as u16;
                    tracker.has_upper = true;
                }
            }
            drop = u32::from(reference.wrapping_sub(plateau) as u16);
            self.reference = plateau as u16;
            let tracker = &mut self.tracker;
            let bonus = (-60 * strength_effective.cast_signed() / 255 + 100) as u16;
            tracker.score = add_capped(tracker.score, bonus, SCORE_MAX);
            plateau
        };

        self.tracker.stroke_active = false;
        if estimate == u32::from(VALUE_NONE) {
            return;
        }
        if strength == 0 {
            self.publish(estimate, 0, 0, 0);
            return;
        }
        if reference != u32::from(VALUE_NONE) {
            self.tracker
                .update_stability(reference, estimate, plateau, drop, strength);
        }
        self.update_estimate(estimate, plateau, strength);
    }

    fn update_estimate(&mut self, estimate: u32, plateau: u32, strength: u32) {
        let has_upper = self
            .tracker
            .update_upper_candidate(estimate, plateau, strength);
        let tracker = &mut self.tracker;
        let anchor = if has_upper {
            u32::from(tracker.upper)
        } else {
            estimate
        };
        let knee = anchor.max(MARGIN_KNEE);
        let margin = strength * (42 * ((knee - MARGIN_KNEE) & 0xFFFF) / 255) / 255;
        let lower_bound = anchor.wrapping_sub(margin) & 0xFFFF;
        tracker.lower_bound = lower_bound as u16;

        let base = if estimate <= lower_bound {
            if !has_upper {
                self.publish(estimate, strength, 0, strength);
                return;
            }
            estimate
        } else {
            let over = estimate.wrapping_sub(anchor).wrapping_add(margin) & 0xFFFF;
            let threshold_low = 127 * margin / 255;
            let threshold_high = 229 * margin / 255;
            let mut weight = 0;
            if over <= (threshold_low & 0xFFFF) || over < (threshold_high & 0xFFFF) {
                let mut stability = u32::from(tracker.stability);
                if over > (threshold_low & 0xFFFF) {
                    stability = stability.wrapping_sub(
                        (stability * (over - threshold_low))
                            .checked_div(threshold_high - threshold_low)
                            .unwrap_or(0),
                    ) & 0xFFFF;
                }
                if stability > 500 {
                    weight = if stability >= 1000 {
                        255
                    } else {
                        let ramp = 255 * (stability - 500) / 500;
                        ramp * ramp / 255
                    };
                }
            }
            let base = lower_bound.wrapping_add(over * weight / 255) & 0xFFFF;
            if !has_upper {
                self.publish(base, strength, 0, strength);
                return;
            }
            base
        };

        let span = u32::from(self.tracker.upper).wrapping_sub(base);
        let fraction = (255_u32.wrapping_mul(estimate.wrapping_sub(base)))
            .checked_div(span)
            .unwrap_or(0)
            & 0xFF;
        let strength_effective = (strength * fraction / 255) & 0xFF;
        self.publish(
            base,
            strength_effective,
            strength.wrapping_sub(strength_effective) & 0xFFFF,
            strength,
        );
    }
}

impl Tracker {
    fn follow_stroke(&mut self, value: u16, now_us: u16) {
        let velocity = self.velocity;
        let peak = self.stroke_peak_velocity;
        if velocity < -(peak * STROKE_REVERSAL) {
            self.stroke_active = false;
            return;
        }
        if velocity > peak {
            self.stroke_peak_velocity = velocity;
        }
        if velocity.abs() > QUIET_VELOCITY {
            self.quiet_samples = 0;
            return;
        }
        self.quiet_samples = self.quiet_samples.wrapping_add(1);
        if self.quiet_samples < QUIET_SAMPLES {
            return;
        }
        self.plateau_start_us = now_us;
        self.plateau_samples = 0;
        self.plateau_mean = f32::from(value);
        self.plateau_max = value;
        self.plateau_min = value;
        self.plateau_active = true;
    }

    fn extend_plateau(&mut self, value: u16) -> bool {
        let old_mean = self.plateau_mean;
        if self.plateau_max < value {
            self.plateau_max = value;
        }
        if self.plateau_min > value {
            self.plateau_min = value;
        }
        let samples = self.plateau_samples.wrapping_add(1);
        self.plateau_samples = samples;
        let earlier = (u32::from(samples).wrapping_sub(1)).cast_signed() as f32;
        let mean = (old_mean * earlier + f32::from(value)) / f32::from(samples);
        self.plateau_mean = mean;
        let skew =
            ((f32::from(self.plateau_max) - mean) - (mean - f32::from(self.plateau_min))).abs();
        if self.velocity.abs() > PLATEAU_MAX_VELOCITY || (samples != 0 && skew > PLATEAU_MAX_SKEW) {
            self.stroke_active = false;
            self.plateau_active = false;
            return false;
        }
        true
    }

    fn update_stability(
        &mut self,
        reference: u32,
        estimate: u32,
        plateau: u32,
        drop: u32,
        strength: u32,
    ) {
        if drop != 0 {
            let gain = (drop.min(100) + 110) as u16;
            self.stability = add_capped(self.stability, gain, STABILITY_MAX);
            return;
        }
        let tolerance = (30 * strength / 255 + 25) & 0xFFFF;
        if reference > plateau || plateau.cast_signed() > (estimate + tolerance).cast_signed() {
            return;
        }
        self.stability = add_capped(self.stability, 25, STABILITY_MAX);
    }

    fn update_upper_candidate(&mut self, estimate: u32, plateau: u32, strength: u32) -> bool {
        let upper = if self.has_upper {
            u32::from(self.upper)
        } else {
            estimate
        };
        let mut hits = self.candidate_hits;
        let candidate_base;
        let mut candidate_max;
        if hits != 0 {
            candidate_base = u32::from(self.candidate_base);
            if plateau < candidate_base {
                let tolerance = strength * (29 * candidate_base / 255) / 255;
                if (candidate_base - plateau).cast_signed() > tolerance.cast_signed() {
                    let misses = self.candidate_misses.wrapping_add(1);
                    self.candidate_misses = misses;
                    if misses == CANDIDATE_MISSES {
                        hits -= 1;
                        self.candidate_hits = hits;
                        self.candidate_misses = 0;
                        if hits == 0 {
                            self.candidate_base = 0;
                            self.candidate_max = 0;
                        }
                    }
                }
                return self.has_upper;
            }
            candidate_max = u32::from(self.candidate_max);
        } else {
            if plateau <= upper {
                return self.has_upper;
            }
            self.candidate_base = plateau as u16;
            self.candidate_max = 0;
            candidate_base = plateau;
            candidate_max = 0;
        }

        hits = hits.wrapping_add(1);
        self.candidate_misses = 0;
        if plateau > candidate_max {
            candidate_max = plateau;
            self.candidate_max = plateau as u16;
        }
        let hits_squared = u32::from(hits) * u32::from(hits);
        let promoted = candidate_max.wrapping_sub(
            (5 * candidate_max / 255)
                .checked_div(hits_squared)
                .unwrap_or(0),
        ) & 0xFFFF;
        self.candidate_hits = hits;

        if upper < promoted {
            self.upper = promoted as u16;
            self.has_upper = true;
            if promoted > candidate_base {
                self.candidate_base = candidate_max as u16;
                self.candidate_hits = 1;
                return true;
            }
        }
        if hits == CANDIDATE_HITS {
            self.has_upper = true;
            self.upper = candidate_base as u16;
            if candidate_max > candidate_base {
                self.candidate_base = candidate_max as u16;
                self.candidate_hits = 1;
            } else {
                self.candidate_base = 0;
                self.candidate_max = 0;
                self.candidate_hits = 0;
            }
            return true;
        }
        self.has_upper
    }
}

fn add_capped(value: u16, gain: u16, cap: u16) -> u16 {
    if i32::from(value) > i32::from(cap) - i32::from(gain) {
        cap
    } else {
        value.wrapping_add(gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REST: u16 = 300;
    const STRENGTH: u8 = 255;

    fn stroke(
        tracker: &mut BottomOut,
        bottom: &mut u16,
        samples: impl IntoIterator<Item = (u16, u16)>,
    ) {
        let mut previous = 0;
        for (value, now_us) in samples {
            if value > REST + 5 {
                if previous <= REST + 5 {
                    tracker.note_press_edge(previous, now_us);
                }
                tracker.observe(value, now_us, STRENGTH, REST, *bottom);
                *bottom = tracker.bottom_level(value, REST, *bottom);
            }
            previous = value;
        }
    }

    fn press(depth: u16, start_us: u16) -> impl Iterator<Item = (u16, u16)> {
        let steps = [310, 500, 800, 1000]
            .into_iter()
            .map(move |value| value.min(depth));
        steps
            .chain(core::iter::once(depth))
            .chain((0..25).map(move |sample| depth + sample % 2))
            .zip((0..).map(move |sample: u16| start_us.wrapping_add(1100 * sample)))
    }

    #[test]
    fn a_held_fast_press_becomes_the_learned_bottom_out_level() {
        let mut tracker = BottomOut::new();
        let mut bottom = 1200;
        stroke(&mut tracker, &mut bottom, press(1100, 0));
        assert_eq!(bottom, 973);
        assert_eq!(tracker.estimate(), Some(973));
        assert_eq!(tracker.reference(), Some(1100));
        assert!(tracker.is_learned());
        assert_eq!(tracker.confidence(), 40);
    }

    #[test]
    fn a_shallower_plateau_lowers_the_reference_and_raises_the_confidence() {
        let mut tracker = BottomOut::new();
        let mut bottom = 1200;
        let samples = press(1100, 0)
            .chain(core::iter::once((REST, 40_000)))
            .chain(press(1050, 41_100));
        stroke(&mut tracker, &mut bottom, samples);
        assert_eq!(tracker.reference(), Some(1050));
        assert_eq!(tracker.estimate(), Some(973));
        assert_eq!(tracker.confidence(), 91);
        assert_eq!(bottom, 973);
    }

    #[test]
    fn samples_closer_than_a_millisecond_are_ignored() {
        let mut tracker = BottomOut::new();
        tracker.observe(400, 0, STRENGTH, REST, 1200);
        tracker.observe(900, 1000, STRENGTH, REST, 1200);
        assert_eq!(tracker.tracker.last_sample_value, 400);
        tracker.observe(900, 1001, STRENGTH, REST, 1200);
        assert_eq!(tracker.tracker.last_sample_value, 900);
    }
}
