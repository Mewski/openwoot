use super::{
    ADDRESSES_PER_INPUT, Converter, Event, Events, Map, MatrixKey, Position, Recheck, Reference,
    Sensing, Travel,
};
use crate::analog::{
    Advance, Decision, NoiseState, NoiseStatistics, ReboundState, Recalibration, Sensor, Switch,
};

const SWEEP_SAMPLES: u8 = 100;
const SETTLE_CYCLES: u8 = 3;
const DELAY_FIRST_US: u16 = 254;
const DELAY_ADDRESS_US: u16 = 149;
const DELAY_INPUT_US: u16 = 54;
const DELAY_SAME_INPUT_US: u16 = 2;
const DELAY_SWEEP_REPEAT_US: u16 = 1;
const CYCLE_PERIOD_US: u16 = 900;
const CYCLE_START_PARALLEL_TICKS: u16 = 3;
const CYCLE_START_SINGLE_TICKS: u16 = 55;
const ACTIVITY_MARGIN: u32 = 20;
const SWEEP_BAND_FACTOR: f32 = 1.2;
const SWEEP_SMALL_BAND: u16 = 4;
const SWEEP_PARALLEL_REST_MARGIN: u16 = 5;
const SWEEP_MINIMUM_RESET: u16 = 0x0FFF;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Single,
    Parallel,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sweep {
    samples_left: u8,
    parallel: bool,
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
    successors: [[Position; ADDRESSES_PER_INPUT]; INPUTS],
    map: Map<INPUTS, ADDRESSES>,
    recalibration: Recalibration,
    sweep_done: bool,
    parallel_ready: bool,
    applied: Option<Mode>,
    requested: Mode,
    mode_change_pending: bool,
    settle_cycles: u8,
    events: Events,
    sensors: [[Sensor<S::Noise>; ADDRESSES_PER_INPUT]; INPUTS],
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    #[must_use]
    pub fn new(converter: S::Converter, map: Map<INPUTS, ADDRESSES>, order: [u8; INPUTS]) -> Self {
        const { assert!(INPUTS > 0 && INPUTS <= 16 && ADDRESSES > 0 && ADDRESSES <= 16) };
        Self {
            converter,
            map,
            successors: successors(&map),
            order,
            sensors: core::array::from_fn(|_| core::array::from_fn(|_| Sensor::new())),
            recheck: S::Recheck::default(),
            reference: S::Reference::default(),
            recalibration: Recalibration::new(),
            events: Events::new(),
            sweep: None,
            sweep_done: false,
            parallel_ready: false,
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
        self.sensors
            .get(position.row())
            .and_then(|sensors| sensors.get(position.column()))
    }

    pub fn sensor_mut(&mut self, position: Position) -> Option<&mut Sensor<S::Noise>> {
        sensor_at(&mut self.sensors, position)
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

    pub fn set_raw_scanning(&mut self, enabled: bool) {
        self.raw_scanning = enabled;
    }

    pub fn set_rebound_filter(&mut self, enabled: bool) {
        if enabled && !self.rebound_filter {
            for sensor in self.sensors.iter_mut().flatten() {
                sensor.rebound = crate::analog::Rebound::new();
            }
        }
        self.rebound_filter = enabled;
    }

    pub fn apply_switches(&mut self, mut switch_for: impl FnMut(MatrixKey) -> Switch) {
        for (position, key) in self.map.iter() {
            if let Some(sensor) = sensor_at(&mut self.sensors, position) {
                sensor.set_switch(switch_for(key));
            }
        }
    }

    pub fn start_sweep(&mut self) {
        if self.mode_is_parallel() && self.sweep.is_none() {
            self.set_mode(Mode::Single);
        }
        for sensor in self.sensors.iter_mut().flatten() {
            sensor.reset_for_sweep();
        }
        self.sweep = Some(Sweep {
            samples_left: SWEEP_SAMPLES,
            parallel: false,
        });
        self.sweep_done = false;
        self.converter.select_input(self.input, 0);
        self.converter.select_address(0);
        self.input = 0;
        self.address = 0;
        self.trigger = Some(Trigger::After(DELAY_FIRST_US));
    }

    pub fn prepare(&mut self, now_ms: u32) {
        self.now_ms = now_ms;
        self.events.clear();
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

    #[must_use]
    pub const fn has_trigger(&self) -> bool {
        self.trigger.is_some()
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
                self.parallel_ready = false;
                self.converter.enter_parallel(&self.order, self.address);
                self.parallel_ready = true;
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
        for sensor in self.sensors.iter_mut().flatten() {
            let calibration = match mode {
                Mode::Single => sensor.single,
                Mode::Parallel => sensor.parallel,
            };
            sensor.rest_level = calibration.rest_level;
            sensor.noise_band = calibration.noise_band;
            sensor.value = 0;
            sensor.filtered = 0;
        }
        self.applied = Some(mode);
    }

    fn stop_sampling(&mut self) {
        if self.recalibration.stop() {
            for sensor in self.sensors.iter_mut().flatten() {
                sensor.recalibrating = false;
            }
        }
    }

    fn batch_positions(&self) -> impl Iterator<Item = Position> + use<S, INPUTS, ADDRESSES> {
        let map = self.map;
        self.recalibration
            .batching()
            .members(self.recalibration.batch(), INPUTS, ADDRESSES)
            .map(|(input, address)| Position::new(input, address))
            .filter(move |&position| map.key(position).is_some())
    }

    fn restart_batch(&mut self) {
        for position in self.batch_positions() {
            if let Some(sensor) = sensor_at(&mut self.sensors, position) {
                sensor.noise.restart();
            }
        }
    }

    fn poll_batch(&mut self) -> bool {
        let mut sampling = false;
        for position in self.batch_positions() {
            let Some(sensor) = sensor_at(&mut self.sensors, position) else {
                continue;
            };
            match sensor.noise.state() {
                NoiseState::Sampling => {
                    if !sensor.recalibrating {
                        sensor.noise.restart();
                        sensor.recalibrating = true;
                    }
                    sampling = true;
                }
                NoiseState::Idle | NoiseState::Unstable => {
                    sensor.noise.restart();
                    sensor.recalibrating = true;
                    sampling = true;
                }
                NoiseState::Done => {}
            }
        }
        sampling
    }

    fn advance_batch(&mut self, now_ms: u32) {
        match self
            .recalibration
            .advance(self.mode, now_ms, INPUTS, ADDRESSES)
        {
            Advance::NextBatch => self.restart_batch(),
            Advance::PassComplete => {
                for sensor in self.sensors.iter_mut().flatten() {
                    sensor.recalibrating = false;
                }
            }
        }
    }

    #[inline(never)]
    fn sweep_step(&mut self, sweep: Sweep) -> Outcome {
        let samples_left = if sweep.parallel && !self.parallel_ready {
            sweep.samples_left
        } else {
            self.track_sweep_samples(sweep)
        };
        let samples_left = samples_left.wrapping_sub(1);
        if samples_left != 0 {
            self.sweep = Some(Sweep {
                samples_left,
                ..sweep
            });
            self.converter.trigger_after(DELAY_SWEEP_REPEAT_US);
            return Outcome::Running;
        }
        let mut next = Sweep {
            samples_left: SWEEP_SAMPLES,
            ..sweep
        };
        if usize::from(self.address) + 1 < ADDRESSES {
            self.address += 1;
            self.sweep = Some(next);
            self.converter.select_address(self.address);
            let unkeyed = self
                .map
                .key(Position::new(self.input, self.address))
                .is_none();
            self.converter.trigger_after(if !sweep.parallel && unkeyed {
                DELAY_SAME_INPUT_US
            } else {
                DELAY_ADDRESS_US
            });
            return Outcome::Running;
        }
        self.address = 0;
        self.converter.select_address(0);
        if sweep.parallel {
            self.sweep = None;
            self.sweep_done = true;
            return Outcome::Finished;
        }
        let previous = self.input;
        let input = if usize::from(previous) + 1 < INPUTS {
            previous + 1
        } else {
            0
        };
        self.converter.select_input(previous, input);
        self.input = input;
        if input == 0 {
            next.parallel = true;
            if self.parallel_ready {
                self.converter.sweep_enter_parallel(None);
            } else {
                self.converter.sweep_enter_parallel(Some(&self.order));
                self.parallel_ready = true;
            }
        }
        self.sweep = Some(next);
        self.converter.trigger_after(DELAY_INPUT_US);
        Outcome::Running
    }

    fn track_sweep_samples(&mut self, sweep: Sweep) -> u8 {
        let ranks = if sweep.parallel { INPUTS } else { 1 };
        for rank in 0..ranks {
            let input = if sweep.parallel {
                self.order.get(rank).copied().unwrap_or_default()
            } else {
                self.input
            };
            let position = Position::new(input, self.address);
            if self.map.key(position).is_none() {
                continue;
            }
            let sample = self.converter.sample(rank);
            self.reference.latch(sample);
            let magnitude = self.reference.magnitude(sample);
            let Some(sensor) = sensor_at(&mut self.sensors, position) else {
                continue;
            };
            if sensor.rest_level < magnitude {
                sensor.rest_level = magnitude;
            } else if sensor.noise_band > magnitude {
                sensor.noise_band = magnitude;
            }
            if sweep.samples_left == 1 {
                finish_sweep_key(sensor, sweep.parallel);
            }
        }
        sweep.samples_left
    }

    fn single_sample(&mut self, travel: &Travel<INPUTS, ADDRESSES>) -> Outcome {
        let sample = self.converter.sample(0);
        let converted = self.pending;
        if converted.is_some_and(|position| position.index() == 0) {
            self.reference.latch(sample);
        }
        let magnitude = self.reference.magnitude(sample);
        let context = self.context();
        let mut sensor = converted.and_then(|position| sensor_at(&mut self.sensors, position));
        let mut value = 0;
        if let Some(sensor) = sensor.as_deref_mut() {
            value = hysteresis(magnitude, sensor.filtered, sensor.noise_band);
            if self
                .recheck
                .reconvert_single(self.input, value, sensor.filtered)
            {
                self.converter.trigger_after(DELAY_SAME_INPUT_US);
                return Outcome::Running;
            }
            sensor.filtered = value;
        }

        let next = successor(&self.successors, self.input, self.address);
        self.address = next.address();
        self.converter.select_address(next.address());
        let previous = self.input;
        let mut cycle_end = false;
        if previous == next.input() {
            self.converter.trigger_after(DELAY_SAME_INPUT_US);
        } else {
            self.converter.select_input(previous, next.input());
            self.input = next.input();
            if next.input() == 0 {
                cycle_end = true;
            } else {
                self.converter.trigger_after(DELAY_INPUT_US);
            }
        }

        let outcome =
            sensor.map(|sensor| process_key(sensor, &self.converter, context, magnitude, value));
        self.pending = Some(next);
        if let (Some(position), Some(outcome)) = (converted, outcome) {
            self.finish_key(position, outcome, travel);
        }
        if cycle_end {
            self.converter.end_cycle(CYCLE_PERIOD_US);
            return Outcome::Finished;
        }
        Outcome::Running
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

    fn parallel_samples(&mut self, travel: &Travel<INPUTS, ADDRESSES>) -> Outcome {
        let address = self.address;
        if self.recheck.is_second_conversion() {
            let first_input = self.recheck.first_input();
            for rank in 0..INPUTS {
                let input = self.order.get(rank).copied().unwrap_or_default();
                let position = Position::new(input, address);
                if input < first_input
                    || !self.recheck.is_deferred(input)
                    || self.map.key(position).is_none()
                {
                    continue;
                }
                let magnitude = self.reference.magnitude(self.converter.sample(rank));
                self.store_parallel(rank, position, magnitude);
            }
        } else {
            for rank in 0..INPUTS {
                let input = self.order.get(rank).copied().unwrap_or_default();
                let position = Position::new(input, address);
                if self.map.key(position).is_none() {
                    continue;
                }
                let sample = self.converter.sample(rank);
                if position.index() == 0 {
                    self.reference.latch(sample);
                }
                let magnitude = self.reference.magnitude(sample);
                let Some(sensor) = sensor_at(&mut self.sensors, position) else {
                    continue;
                };
                let value = hysteresis(magnitude, sensor.filtered, sensor.noise_band);
                if self.recheck.defer(input, value, sensor.filtered) {
                    continue;
                }
                sensor.filtered = value;
                set(&mut self.parallel_values, rank, value);
                set(&mut self.parallel_magnitudes, rank, magnitude);
            }
            if self.recheck.reconvert_deferred() {
                self.converter.trigger_after(DELAY_SAME_INPUT_US);
                return Outcome::Running;
            }
        }

        let next = if usize::from(address) + 1 < ADDRESSES {
            address + 1
        } else {
            0
        };
        self.address = next;
        self.recheck.reset();
        self.converter.select_address(next);
        let cycle_end = next == 0;
        if !cycle_end {
            self.converter.trigger_after(DELAY_SAME_INPUT_US);
        }

        for rank in 0..INPUTS {
            let input = self.order.get(rank).copied().unwrap_or_default();
            let position = Position::new(input, address);
            let magnitude = self
                .parallel_magnitudes
                .get(rank)
                .copied()
                .unwrap_or_default();
            let value = self.parallel_values.get(rank).copied().unwrap_or_default();
            self.process(position, magnitude, value, travel);
        }

        if cycle_end {
            self.converter.end_cycle(CYCLE_PERIOD_US);
            return Outcome::Finished;
        }
        Outcome::Running
    }

    fn store_parallel(&mut self, rank: usize, position: Position, magnitude: u16) {
        if let Some(sensor) = sensor_at(&mut self.sensors, position) {
            let value = hysteresis(magnitude, sensor.filtered, sensor.noise_band);
            sensor.filtered = value;
            set(&mut self.parallel_values, rank, value);
            set(&mut self.parallel_magnitudes, rank, magnitude);
        }
    }

    #[inline(always)]
    fn process(
        &mut self,
        position: Position,
        magnitude: u16,
        value: u16,
        travel: &Travel<INPUTS, ADDRESSES>,
    ) {
        if self.map.key(position).is_none() {
            return;
        }
        let context = self.context();
        let Some(sensor) = sensor_at(&mut self.sensors, position) else {
            return;
        };
        let outcome = process_key(sensor, &self.converter, context, magnitude, value);
        self.finish_key(position, outcome, travel);
    }

    #[inline(always)]
    fn finish_key(
        &mut self,
        position: Position,
        outcome: KeyOutcome,
        travel: &Travel<INPUTS, ADDRESSES>,
    ) {
        if let Some(key_travel) = outcome.travel
            && let Some(key) = self.map.key(position)
        {
            travel.set(position, key_travel);
            self.events.push(Event {
                position,
                key,
                travel: key_travel,
            });
        }
        if outcome.activity {
            self.note_activity();
        }
    }

    fn note_activity(&mut self) {
        self.recalibration.hold(self.now_ms);
        self.stop_sampling();
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

const fn is_calibrated<Noise>(sensor: &Sensor<Noise>) -> bool {
    sensor.noise_band != 0 && sensor.rest_level != 0
}

#[inline(always)]
fn hysteresis(magnitude: u16, filtered: u16, band: u16) -> u16 {
    let difference = i32::from(magnitude) - i32::from(filtered);
    if difference > i32::from(band) {
        magnitude - band
    } else if difference < -i32::from(band) {
        magnitude.wrapping_add(band)
    } else {
        filtered
    }
}

#[inline(always)]
fn sensor_at<Noise, const INPUTS: usize>(
    sensors: &mut [[Sensor<Noise>; ADDRESSES_PER_INPUT]; INPUTS],
    position: Position,
) -> Option<&mut Sensor<Noise>> {
    sensors
        .get_mut(position.row())
        .and_then(|sensors| sensors.get_mut(position.column()))
}

#[inline(always)]
fn set<const INPUTS: usize>(values: &mut [u16; INPUTS], rank: usize, value: u16) {
    if let Some(slot) = values.get_mut(rank) {
        *slot = value;
    }
}

fn finish_sweep_key<Noise>(sensor: &mut Sensor<Noise>, parallel: bool) {
    let mut rest = sensor.rest_level;
    let spread = f32::from(rest) - f32::from(sensor.noise_band);
    let band = (libm::ceilf(spread * 0.5) * SWEEP_BAND_FACTOR) as u32 as u16;
    sensor.rest_level = 0;
    sensor.noise_band = SWEEP_MINIMUM_RESET;
    if parallel {
        sensor.parallel.noise_band = band;
        sensor.parallel.rest_level = if band > SWEEP_SMALL_BAND {
            rest
        } else {
            rest.wrapping_add(SWEEP_PARALLEL_REST_MARGIN)
        };
        rest = sensor.single.rest_level;
    } else {
        sensor.single.noise_band = band;
        sensor.single.rest_level = rest;
    }
    sensor.bottom_level = sensor.switch.bottom_level(rest);
    sensor.bottom_out.reset();
}

fn successors<const INPUTS: usize, const ADDRESSES: usize>(
    map: &Map<INPUTS, ADDRESSES>,
) -> [[Position; ADDRESSES_PER_INPUT]; INPUTS] {
    core::array::from_fn(|start_input| {
        core::array::from_fn(|start_address| {
            let mut input = start_input;
            let mut address = start_address;
            for _ in 0..INPUTS * ADDRESSES {
                address += 1;
                if address >= ADDRESSES {
                    address = 0;
                    input = (input + 1) % INPUTS;
                }
                let position = Position::new(input as u8, address as u8);
                if map.key(position).is_some() {
                    return position;
                }
            }
            Position::new(input as u8, address as u8)
        })
    })
}

#[inline(never)]
fn feed_noise<Noise: NoiseStatistics>(
    sensor: &mut Sensor<Noise>,
    magnitude: u16,
    mode: Mode,
) -> bool {
    match sensor.noise.add(magnitude) {
        NoiseState::Done => {
            sensor.recalibrating = false;
            let calibration = sensor.noise.calibration(sensor.noise_band);
            sensor.noise_band = calibration.noise_band;
            sensor.rest_level = calibration.rest_level;
            match mode {
                Mode::Single => sensor.single = calibration,
                Mode::Parallel => sensor.parallel = calibration,
            }
            false
        }
        NoiseState::Unstable => is_calibrated(sensor),
        NoiseState::Idle | NoiseState::Sampling => false,
    }
}

#[inline(never)]
fn track_press<Noise: NoiseStatistics, C: Converter>(
    sensor: &mut Sensor<Noise>,
    converter: &C,
    previous: u16,
    value: u16,
    rebound_filter: bool,
) -> bool {
    let threshold = sensor.rest_threshold();
    let now_us = converter.micros();
    if previous <= threshold {
        sensor.bottom_out.note_press_edge(previous, now_us);
    }
    sensor.bottom_out.observe(
        value,
        now_us,
        sensor.switch.learn_strength(),
        sensor.rest_level,
        sensor.bottom_level,
    );
    sensor.bottom_level =
        sensor
            .bottom_out
            .bottom_level(value, sensor.rest_level, sensor.bottom_level);
    rebound_filter
        && sensor
            .rebound
            .detect(threshold, sensor.bottom_level, value, converter.micros())
}

#[derive(Clone, Copy, Debug)]
struct Context {
    raw_scanning: bool,
    settling: bool,
    rebound_filter: bool,
    mode: Mode,
}

#[derive(Clone, Copy, Debug)]
struct KeyOutcome {
    travel: Option<u16>,
    activity: bool,
}

#[inline(always)]
fn successor<const INPUTS: usize>(
    successors: &[[Position; ADDRESSES_PER_INPUT]; INPUTS],
    input: u8,
    address: u8,
) -> Position {
    let from = Position::new(input, address);
    successors
        .get(from.row())
        .and_then(|row| row.get(from.column()))
        .copied()
        .unwrap_or(Position::new(0, 0))
}

#[inline(always)]
fn process_key<Noise: NoiseStatistics, C: Converter>(
    sensor: &mut Sensor<Noise>,
    converter: &C,
    context: Context,
    magnitude: u16,
    value: u16,
) -> KeyOutcome {
    let quiet = KeyOutcome {
        travel: None,
        activity: false,
    };
    if context.raw_scanning {
        sensor.value = magnitude;
        return quiet;
    }
    let previous = sensor.value;
    sensor.value = value;
    let mut activity = sensor.recalibrating && feed_noise(sensor, magnitude, context.mode);
    if context.settling {
        return KeyOutcome { activity, ..quiet };
    }

    let threshold = sensor.rest_threshold();
    if u32::from(value) > u32::from(threshold) + ACTIVITY_MARGIN && is_calibrated(sensor) {
        activity = true;
    }
    let mut rebound = false;
    if value > threshold {
        rebound = track_press(sensor, converter, previous, value, context.rebound_filter);
    } else {
        if context.rebound_filter && sensor.rebound.state() != ReboundState::Idle {
            rebound =
                sensor
                    .rebound
                    .detect(threshold, sensor.bottom_level, value, converter.micros());
        }
        if previous <= threshold && !sensor.holds_events {
            return KeyOutcome { activity, ..quiet };
        }
    }
    KeyOutcome {
        travel: Some(if rebound { 0 } else { sensor.travel() }),
        activity,
    }
}
