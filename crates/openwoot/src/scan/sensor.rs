use super::{Converter, LowerRest, Mode};
use crate::analog::{
    BottomOut, Calibration, NoiseState, NoiseStatistics, Rebound, ReboundState, Switch, SwitchModel,
};

const NOISE_BAND_UNMEASURED: u16 = 0xFFFF;
const ACTIVITY_MARGIN: u32 = 20;
const SWEEP_BAND_FACTOR: f32 = 1.2;
const SWEEP_SMALL_BAND: u16 = 4;
const SWEEP_QUIET_BAND: u16 = 1;
const SWEEP_PARALLEL_REST_MARGIN: u16 = 5;
const SWEEP_MINIMUM_RESET: u16 = 0x0FFF;
const UNSTABLE_WINDOWS_SKIPPED: u8 = 3;
const STILL_ACTIVITY_SAMPLES: u16 = 10_000;

#[derive(Clone, Copy, Debug)]
pub(super) struct Context {
    pub(super) raw_scanning: bool,
    pub(super) settling: bool,
    pub(super) rebound_filter: bool,
    pub(super) mode: Mode,
    pub(super) now_ms: u16,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Update {
    pub(super) travel: Option<u16>,
    pub(super) activity: bool,
}

#[derive(Clone, Debug)]
pub struct Sensor<Noise> {
    value: u16,
    filtered: u16,
    rest_level: u16,
    noise_band: u16,
    bottom_level: u16,
    single: Calibration,
    parallel: Calibration,
    bottom_out: BottomOut,
    rebound: Rebound,
    noise: Noise,
    recalibrating: bool,
    switch: Switch,
    holds_events: bool,
    reported: bool,
    unstable_windows: u8,
    still_samples: u16,
    lower_rest: LowerRest,
}

impl<Noise: NoiseStatistics> Default for Sensor<Noise> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Noise: NoiseStatistics> Sensor<Noise> {
    #[must_use]
    pub const fn new() -> Self {
        let unmeasured = Calibration {
            rest_level: 0,
            noise_band: 0,
        };
        Self {
            value: 0,
            filtered: 0,
            rest_level: 0,
            noise_band: NOISE_BAND_UNMEASURED,
            bottom_level: 0,
            single: unmeasured,
            parallel: unmeasured,
            bottom_out: BottomOut::new(),
            rebound: Rebound::new(),
            noise: Noise::IDLE,
            recalibrating: false,
            switch: Switch::untuned(SwitchModel::DEFAULT),
            holds_events: false,
            reported: false,
            unstable_windows: 0,
            still_samples: 0,
            lower_rest: LowerRest::new(),
        }
    }

    #[must_use]
    pub const fn value(&self) -> u16 {
        self.value
    }

    #[must_use]
    pub const fn rest_level(&self) -> u16 {
        self.rest_level
    }

    #[must_use]
    pub const fn bottom_level(&self) -> u16 {
        self.bottom_level
    }

    #[must_use]
    pub const fn single(&self) -> Calibration {
        self.single
    }

    #[must_use]
    pub const fn parallel(&self) -> Calibration {
        self.parallel
    }

    #[must_use]
    pub const fn rebound(&self) -> &Rebound {
        &self.rebound
    }

    #[must_use]
    pub const fn noise(&self) -> &Noise {
        &self.noise
    }

    #[must_use]
    pub const fn switch(&self) -> &Switch {
        &self.switch
    }

    #[must_use]
    pub const fn is_recalibrating(&self) -> bool {
        self.recalibrating
    }

    #[must_use]
    pub fn travel(&self) -> u16 {
        self.switch
            .travel(self.value, self.rest_level, self.bottom_level)
    }

    pub const fn hold_events(&mut self, holds: bool) {
        self.holds_events = holds;
    }

    pub(super) fn set_switch(&mut self, switch: Switch) {
        self.switch = switch;
        self.bottom_level = self.switch.bottom_level(self.single.rest_level);
        self.bottom_out.reset();
    }

    pub(super) const fn reset_rebound(&mut self) {
        self.rebound = Rebound::new();
    }

    pub(super) fn reset_for_sweep(&mut self) {
        self.value = 0;
        self.rest_level = 0;
        self.noise_band = NOISE_BAND_UNMEASURED;
        self.bottom_out.reset();
        self.rebound = Rebound::new();
        self.recalibrating = false;
        self.unstable_windows = 0;
        self.lower_rest.clear();
    }

    pub(super) const fn track_sweep(&mut self, magnitude: u16) {
        if self.rest_level < magnitude {
            self.rest_level = magnitude;
        }
        if self.noise_band > magnitude {
            self.noise_band = magnitude;
        }
    }

    pub(super) fn finish_sweep(&mut self, mode: Mode) {
        let mut rest = self.rest_level;
        let spread = f32::from(rest) - f32::from(self.noise_band);
        let measured = (libm::ceilf(spread * 0.5) * SWEEP_BAND_FACTOR) as u32 as u16;
        let band = if measured == 0 && rest != 0 {
            SWEEP_QUIET_BAND
        } else {
            measured
        };
        self.rest_level = 0;
        self.noise_band = SWEEP_MINIMUM_RESET;
        match mode {
            Mode::Parallel => {
                self.parallel.noise_band = band;
                self.parallel.rest_level = if band > SWEEP_SMALL_BAND {
                    rest
                } else {
                    rest.wrapping_add(SWEEP_PARALLEL_REST_MARGIN)
                };
                rest = self.single.rest_level;
            }
            Mode::Single => {
                self.single.noise_band = band;
                self.single.rest_level = rest;
            }
        }
        self.bottom_level = self.switch.bottom_level(rest);
        self.bottom_out.reset();
    }

    pub(super) const fn load_calibration(&mut self, mode: Mode) {
        let calibration = self.calibration(mode);
        self.rest_level = calibration.rest_level;
        self.noise_band = calibration.noise_band;
        self.value = 0;
        self.filtered = 0;
        self.unstable_windows = 0;
        self.lower_rest.clear();
    }

    pub(super) const fn stop_recalibrating(&mut self) {
        self.recalibrating = false;
    }

    pub(super) const fn finish_pass(&mut self) {
        self.recalibrating = false;
        self.unstable_windows = 0;
    }

    pub(super) fn restart_noise(&mut self) {
        self.noise.restart();
    }

    pub(super) fn poll_recalibration(&mut self) -> bool {
        if self.unstable_windows >= UNSTABLE_WINDOWS_SKIPPED || self.is_active() {
            return false;
        }
        match self.noise.state() {
            NoiseState::Sampling => {
                if !self.recalibrating {
                    self.noise.restart();
                    self.recalibrating = true;
                }
                true
            }
            NoiseState::Idle | NoiseState::Unstable => {
                self.noise.restart();
                self.recalibrating = true;
                true
            }
            NoiseState::Done => false,
        }
    }

    #[inline(always)]
    pub(super) fn filter(&self, magnitude: u16) -> u16 {
        let band = self.noise_band;
        let difference = i32::from(magnitude) - i32::from(self.filtered);
        if difference > i32::from(band) {
            magnitude - band
        } else if difference < -i32::from(band) {
            magnitude.wrapping_add(band)
        } else {
            self.filtered
        }
    }

    #[inline(always)]
    pub(super) const fn filtered(&self) -> u16 {
        self.filtered
    }

    #[inline(always)]
    pub(super) const fn set_filtered(&mut self, value: u16) {
        self.filtered = value;
    }

    #[inline(always)]
    pub(super) fn update(
        &mut self,
        converter: &impl Converter,
        context: Context,
        magnitude: u16,
        value: u16,
    ) -> Update {
        let quiet = Update {
            travel: None,
            activity: false,
        };
        if context.raw_scanning {
            self.lower_rest.clear();
            self.value = magnitude;
            return if core::mem::take(&mut self.reported) {
                Update {
                    travel: Some(0),
                    activity: false,
                }
            } else {
                quiet
            };
        }
        let previous = self.value;
        self.value = value;
        self.still_samples = if value == previous {
            self.still_samples.saturating_add(1)
        } else {
            0
        };
        let mut activity = self.recalibrating && self.feed_noise(magnitude, context.mode);
        if context.settling {
            self.lower_rest.clear();
            return Update { activity, ..quiet };
        }
        if let Some(rest_level) =
            self.lower_rest
                .observe(value, self.rest_level, self.noise_band, context.now_ms)
        {
            self.lower_rest_level(rest_level, context.mode);
        }

        let threshold = self.rest_threshold();
        if self.is_active() && self.still_samples < STILL_ACTIVITY_SAMPLES {
            activity = true;
        }
        let mut rebound = false;
        if value > threshold {
            rebound = self.track_press(converter, previous, value, context.rebound_filter);
        } else {
            if context.rebound_filter && self.rebound.state() != ReboundState::Idle {
                rebound =
                    self.rebound
                        .detect(threshold, self.bottom_level, value, converter.micros());
            }
            if previous <= threshold && !self.reported && !self.holds_events {
                return Update { activity, ..quiet };
            }
        }
        let travel = if rebound { 0 } else { self.travel() };
        self.reported = travel != 0;
        Update {
            travel: Some(travel),
            activity,
        }
    }

    #[inline(never)]
    fn feed_noise(&mut self, magnitude: u16, mode: Mode) -> bool {
        match self.noise.add(magnitude) {
            NoiseState::Done => {
                self.recalibrating = false;
                self.unstable_windows = 0;
                let calibration = self.noise.calibration(self.noise_band);
                self.noise_band = calibration.noise_band;
                self.rest_level = calibration.rest_level;
                *self.calibration_mut(mode) = calibration;
                false
            }
            NoiseState::Unstable => {
                self.recalibrating = false;
                self.unstable_windows = self.unstable_windows.saturating_add(1);
                self.is_calibrated()
            }
            NoiseState::Idle | NoiseState::Sampling => false,
        }
    }

    #[inline(never)]
    fn lower_rest_level(&mut self, rest_level: u16, mode: Mode) {
        self.rest_level = rest_level;
        self.calibration_mut(mode).rest_level = rest_level;
        self.bottom_level = self.switch.bottom_level(rest_level);
        self.bottom_out.reset();
    }

    #[inline(never)]
    fn track_press(
        &mut self,
        converter: &impl Converter,
        previous: u16,
        value: u16,
        rebound_filter: bool,
    ) -> bool {
        let threshold = self.rest_threshold();
        let now_us = converter.micros();
        if previous <= threshold {
            self.bottom_out.note_press_edge(previous, now_us);
        }
        self.bottom_out.observe(
            value,
            now_us,
            self.switch.learn_strength(),
            self.rest_level,
            self.bottom_level,
        );
        self.bottom_level = self
            .bottom_out
            .bottom_level(value, self.rest_level, self.bottom_level)
            .max(Switch::lowest_bottom_level(self.rest_level));
        rebound_filter
            && self
                .rebound
                .detect(threshold, self.bottom_level, value, converter.micros())
    }

    const fn calibration(&self, mode: Mode) -> Calibration {
        match mode {
            Mode::Single => self.single,
            Mode::Parallel => self.parallel,
        }
    }

    const fn calibration_mut(&mut self, mode: Mode) -> &mut Calibration {
        match mode {
            Mode::Single => &mut self.single,
            Mode::Parallel => &mut self.parallel,
        }
    }

    const fn is_calibrated(&self) -> bool {
        self.noise_band != 0 && self.rest_level != 0
    }

    fn is_active(&self) -> bool {
        u32::from(self.value) > u32::from(self.rest_threshold()) + ACTIVITY_MARGIN
            && self.is_calibrated()
    }

    const fn rest_threshold(&self) -> u16 {
        self.rest_level.wrapping_add(Switch::REST_MARGIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::MinMax;

    fn unstable_window(sensor: &mut Sensor<MinMax>) {
        for index in 0..1000_u16 {
            if !sensor.is_recalibrating() {
                break;
            }
            let amplitude = index / 50;
            let magnitude = if index % 2 == 0 {
                100 + amplitude
            } else {
                100 - amplitude
            };
            sensor.feed_noise(magnitude, Mode::Single);
        }
    }

    #[test]
    fn a_sensor_whose_windows_stay_unstable_is_left_out_until_the_pass_ends() {
        let mut sensor = Sensor::<MinMax>::new();
        for _ in 0..UNSTABLE_WINDOWS_SKIPPED {
            assert!(sensor.poll_recalibration());
            unstable_window(&mut sensor);
            assert_eq!(sensor.noise().state(), NoiseState::Unstable);
            assert!(!sensor.is_recalibrating());
        }
        assert!(!sensor.poll_recalibration());
        sensor.finish_pass();
        assert!(sensor.poll_recalibration());
    }

    #[test]
    fn a_complete_window_forgets_the_unstable_ones_before_it() {
        let mut sensor = Sensor::<MinMax>::new();
        assert!(sensor.poll_recalibration());
        unstable_window(&mut sensor);
        assert!(sensor.poll_recalibration());
        for index in 0..1000_u16 {
            sensor.feed_noise(100 + index % 3, Mode::Single);
        }
        assert_eq!(sensor.noise().state(), NoiseState::Done);
        assert_eq!(sensor.unstable_windows, 0);
    }
}
