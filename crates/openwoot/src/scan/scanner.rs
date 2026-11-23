mod batch;
mod parallel;
mod single;
mod sweep;

use single::successors;
use sweep::Sweep;

use super::{
    Context, Converter, Decision, Event, Events, Grid, Map, Mode, Position, Recalibration,
    Reference, Sensing, Sensor, Sensors, Travel, Update,
};
use crate::{analog::Switch, keys::Key};

const SETTLE_CYCLES: u8 = 3;
const DELAY_INPUT_US: u16 = 54;
const DELAY_SAME_INPUT_US: u16 = 2;
const CYCLE_PERIOD_US: u16 = 900;
const CYCLE_START_PARALLEL_TICKS: u16 = 3;
const CYCLE_START_SINGLE_TICKS: u16 = 55;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Running,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    After(u16),
    At(u16),
}

#[repr(C)]
pub struct Scanner<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> {
    trigger: Option<Trigger>,
    sweep: Option<Sweep>,
    pending: Option<Position>,
    mode: Mode,
    input: u8,
    address: u8,
    settling: bool,
    raw_scanning: bool,
    rebound_filter: bool,
    now_ms: u32,
    recheck: S::Recheck,
    reference: S::Reference,
    converter: S::Converter,
    order: [u8; INPUTS],
    parallel_magnitudes: [u16; INPUTS],
    parallel_values: [u16; INPUTS],
    successors: Grid<Position, INPUTS>,
    map: Map<INPUTS, ADDRESSES>,
    recalibration: Recalibration,
    sweep_done: bool,
    applied: Option<Mode>,
    requested: Mode,
    mode_change_pending: bool,
    settle_cycles: u8,
    events: Events,
    cycle_finished: bool,
    sensors: &'static mut Sensors<S::Noise, INPUTS>,
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    #[must_use]
    pub fn new(
        converter: S::Converter,
        map: Map<INPUTS, ADDRESSES>,
        order: [u8; INPUTS],
        sensors: &'static mut Sensors<S::Noise, INPUTS>,
    ) -> Self {
        const {
            assert!(
                INPUTS > 0
                    && INPUTS <= 16
                    && ADDRESSES > 0
                    && ADDRESSES <= 16
                    && INPUTS * ADDRESSES <= Events::CAPACITY
            );
        };
        Self {
            converter,
            map,
            successors: successors(&map),
            order,
            sensors,
            recheck: S::Recheck::default(),
            reference: S::Reference::default(),
            recalibration: Recalibration::new(),
            events: Events::new(),
            cycle_finished: true,
            sweep: None,
            sweep_done: false,
            mode: Mode::Single,
            applied: None,
            requested: Mode::Single,
            mode_change_pending: false,
            input: 0,
            address: 0,
            pending: None,
            parallel_magnitudes: [0; INPUTS],
            parallel_values: [0; INPUTS],
            settling: false,
            settle_cycles: 0,
            trigger: None,
            raw_scanning: false,
            rebound_filter: false,
            now_ms: 0,
        }
    }

    #[must_use]
    pub const fn converter(&self) -> &S::Converter {
        &self.converter
    }

    pub const fn converter_mut(&mut self) -> &mut S::Converter {
        &mut self.converter
    }

    #[must_use]
    pub const fn map(&self) -> &Map<INPUTS, ADDRESSES> {
        &self.map
    }

    #[must_use]
    pub const fn events(&self) -> &Events {
        &self.events
    }

    #[must_use]
    pub const fn mode(&self) -> Option<Mode> {
        self.applied
    }

    #[must_use]
    pub const fn recalibration(&self) -> &Recalibration {
        &self.recalibration
    }

    #[must_use]
    pub fn reference_level(&self) -> u16 {
        self.reference.latched()
    }

    #[must_use]
    pub const fn is_calibrating(&self) -> bool {
        self.sweep.is_some()
    }

    #[must_use]
    pub const fn is_settling(&self) -> bool {
        self.settling
    }

    #[must_use]
    pub fn sensor(&self, position: Position) -> Option<&Sensor<S::Noise>> {
        self.sensors.get(position)
    }

    pub fn sensor_mut(&mut self, position: Position) -> Option<&mut Sensor<S::Noise>> {
        self.sensors.get_mut(position)
    }

    pub fn set_map(&mut self, map: Map<INPUTS, ADDRESSES>) {
        self.map = map;
        self.successors = successors(&map);
        self.pending = None;
    }

    pub fn request_mode(&mut self, mode: Mode) {
        if self.requested != mode {
            self.requested = mode;
            self.mode_change_pending = true;
        }
    }

    pub const fn set_raw_scanning(&mut self, enabled: bool) {
        self.raw_scanning = enabled;
    }

    pub fn set_rebound_filter(&mut self, enabled: bool) {
        if enabled && !self.rebound_filter {
            for sensor in self.sensors.iter_mut() {
                sensor.reset_rebound();
            }
        }
        self.rebound_filter = enabled;
    }

    pub fn apply_switches(&mut self, mut switch_for: impl FnMut(Key) -> Switch) {
        for (position, key) in self.map.iter() {
            if let Some(sensor) = self.sensors.get_mut(position) {
                sensor.set_switch(switch_for(key));
            }
        }
    }

    pub fn prepare(&mut self, now_ms: u32) {
        self.now_ms = now_ms;
        if core::mem::take(&mut self.cycle_finished) {
            self.events.clear();
        }
        if self.sweep.is_some() {
            return;
        }
        if core::mem::take(&mut self.sweep_done) {
            self.mode_change_pending = true;
        }
        if self.settling {
            if self.settle_cycles != 0 {
                self.settle_cycles -= 1;
            } else {
                self.settling = false;
            }
        }
        if self.mode_change_pending {
            self.switch_mode();
            return;
        }
        if !self.settling {
            match self.recalibration.decide(now_ms) {
                Decision::Wait => {}
                decision => {
                    if decision == Decision::StartBatch {
                        self.restart_batch();
                    }
                    if !self.poll_batch() {
                        self.advance_batch(now_ms);
                    }
                }
            }
        }
        self.trigger = Some(Trigger::At(if self.mode_is_parallel() {
            CYCLE_START_PARALLEL_TICKS
        } else {
            CYCLE_START_SINGLE_TICKS
        }));
    }

    #[inline(always)]
    pub fn on_conversion(&mut self, travel: &Travel<INPUTS, ADDRESSES>) -> Outcome {
        if let Some(trigger) = self.trigger.take() {
            match trigger {
                Trigger::After(delay_us) => self.converter.trigger_after(delay_us),
                Trigger::At(ticks) => self.converter.trigger_at(ticks),
            }
            return Outcome::Running;
        }
        if let Some(sweep) = self.sweep {
            self.sweep_step(sweep)
        } else if self.mode_is_parallel() {
            self.parallel_samples(travel)
        } else {
            self.single_sample(travel)
        }
    }

    pub(super) const fn has_trigger(&self) -> bool {
        self.trigger.is_some()
    }

    const fn mode_is_parallel(&self) -> bool {
        matches!(self.mode, Mode::Parallel)
    }

    fn switch_mode(&mut self) {
        self.settling = true;
        self.settle_cycles = SETTLE_CYCLES;
        self.set_mode(self.requested);
        self.mode_change_pending = false;
        self.trigger = Some(Trigger::After(DELAY_INPUT_US));
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.address = 0;
        match mode {
            Mode::Parallel => {
                self.converter.enter_parallel(&self.order, self.address);
            }
            Mode::Single => {
                self.converter.enter_single(self.input);
                self.input = 0;
            }
        }
        if self.applied != Some(mode) {
            self.apply_mode_calibration(mode);
        }
    }

    fn apply_mode_calibration(&mut self, mode: Mode) {
        self.stop_sampling();
        self.recalibration.restart(mode);
        for sensor in self.sensors.iter_mut() {
            sensor.load_calibration(mode);
        }
        self.applied = Some(mode);
    }

    #[inline(always)]
    const fn context(&self) -> Context {
        Context {
            raw_scanning: self.raw_scanning,
            settling: self.settling,
            rebound_filter: self.rebound_filter,
            mode: self.mode,
        }
    }

    #[inline(always)]
    fn finish_key(
        &mut self,
        position: Position,
        update: Update,
        travel: &Travel<INPUTS, ADDRESSES>,
    ) {
        if let Some(key_travel) = update.travel
            && let Some(key) = self.map.key(position)
        {
            travel.set(position, key_travel);
            self.events.push(Event {
                position,
                key,
                travel: key_travel,
            });
        }
        if update.activity {
            self.note_activity();
        }
    }
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> core::fmt::Debug
    for Scanner<S, INPUTS, ADDRESSES>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Scanner")
            .field("mode", &self.applied)
            .field("calibrating", &self.sweep.is_some())
            .field("events", &self.events.len())
            .finish_non_exhaustive()
    }
}
