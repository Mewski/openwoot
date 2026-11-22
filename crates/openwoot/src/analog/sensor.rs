use super::{
    BottomOut, Calibration, NoiseStatistics, Rebound, Switch, SwitchType, switch::REST_MARGIN,
};

const NOISE_BAND_UNMEASURED: u16 = 0xFFFF;

#[derive(Clone, Debug)]
pub struct Sensor<Noise> {
    pub(crate) value: u16,
    pub(crate) filtered: u16,
    pub(crate) rest_level: u16,
    pub(crate) noise_band: u16,
    pub(crate) bottom_level: u16,
    pub(crate) single: Calibration,
    pub(crate) parallel: Calibration,
    pub(crate) bottom_out: BottomOut,
    pub(crate) rebound: Rebound,
    pub(crate) noise: Noise,
    pub(crate) recalibrating: bool,
    pub(crate) switch: Switch,
    pub(crate) holds_events: bool,
}

impl<Noise: NoiseStatistics> Default for Sensor<Noise> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Noise: NoiseStatistics> Sensor<Noise> {
    #[must_use]
    pub fn new() -> Self {
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
            noise: Noise::default(),
            recalibrating: false,
            switch: Switch::new(SwitchType::DEFAULT, None),
            holds_events: false,
        }
    }

    #[must_use]
    pub const fn filtered(&self) -> u16 {
        self.filtered
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
    pub const fn noise_band(&self) -> u16 {
        self.noise_band
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
    pub const fn bottom_out(&self) -> &BottomOut {
        &self.bottom_out
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
    pub const fn rest_threshold(&self) -> u16 {
        self.rest_level.wrapping_add(REST_MARGIN)
    }

    #[must_use]
    pub fn travel(&self) -> u16 {
        self.switch
            .travel(self.value, self.rest_level, self.bottom_level)
    }

    pub fn hold_events(&mut self, holds: bool) {
        self.holds_events = holds;
    }

    pub(crate) fn set_switch(&mut self, switch: Switch) {
        self.switch = switch;
        self.reset_bottom(self.single.rest_level);
    }

    pub(crate) fn reset_bottom(&mut self, rest_level: u16) {
        self.bottom_level = self.switch.bottom_level(rest_level);
        self.bottom_out.reset();
    }

    pub(crate) fn reset_for_sweep(&mut self) {
        self.value = 0;
        self.rest_level = 0;
        self.noise_band = NOISE_BAND_UNMEASURED;
        self.bottom_out.reset();
        self.rebound = Rebound::new();
        self.recalibrating = false;
    }
}
