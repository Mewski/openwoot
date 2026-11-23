use openwoot::{
    analog::{MinMax, StandardDeviation},
    scan::{
        ADDRESSES_PER_INPUT, Converter, MeasuredReference, RecheckFrom, Sample, Sensing, ZeroLevel,
    },
};

const ZERO_LEVEL: u16 = 2048;
const CONVERSION_US: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operation {
    SelectAddress(u8),
    SelectInput(u8, u8),
    EnterSingle(u8),
    EnterParallel(u8),
    SweepEnterParallel(bool),
    TriggerAfter(u16),
    TriggerAt(u16),
    EndCycle(u16),
    Recover,
}

#[derive(Debug)]
pub(crate) struct Simulated<const INPUTS: usize> {
    pub(super) readings: [[u16; ADDRESSES_PER_INPUT]; INPUTS],
    pub(super) reference: u16,
    pub(crate) operations: Vec<Operation>,
    pub(super) now_us: u16,
    pub(super) noise: Option<fn(u32) -> i16>,
    conversions: u32,
    address: u8,
    input: u8,
    parallel: bool,
    enabled: [bool; INPUTS],
    order: Vec<u8>,
}

impl<const INPUTS: usize> Default for Simulated<INPUTS> {
    fn default() -> Self {
        Self {
            readings: [[ZERO_LEVEL; ADDRESSES_PER_INPUT]; INPUTS],
            reference: 0,
            operations: Vec::new(),
            now_us: 0,
            noise: None,
            conversions: 0,
            address: 0,
            input: 0,
            parallel: false,
            enabled: [true; INPUTS],
            order: (0..INPUTS)
                .map(|input| u8::try_from(input).unwrap_or(u8::MAX))
                .collect(),
        }
    }
}

impl<const INPUTS: usize> Simulated<INPUTS> {
    pub(crate) fn press(&mut self, input: u8, address: u8, magnitude: u16) {
        if let Some(reading) = self
            .readings
            .get_mut(usize::from(input))
            .and_then(|readings| readings.get_mut(usize::from(address)))
        {
            *reading = ZERO_LEVEL + magnitude;
        }
    }

    pub(super) fn clear(&mut self) {
        self.operations.clear();
    }

    fn reading(&self, input: u8) -> u16 {
        let enabled = self
            .enabled
            .get(usize::from(input))
            .copied()
            .unwrap_or(false);
        let reading = self
            .readings
            .get(usize::from(input))
            .and_then(|readings| readings.get(usize::from(self.address)))
            .copied()
            .filter(|_| enabled)
            .unwrap_or(ZERO_LEVEL);
        let wobble = self.noise.map_or(0, |noise| noise(self.conversions));
        reading.saturating_add_signed(wobble)
    }

    fn enable(&mut self, input: u8, enabled: bool) {
        if let Some(line) = self.enabled.get_mut(usize::from(input)) {
            *line = enabled;
        }
    }

    fn advance(&mut self, delay_us: u16) {
        self.now_us = self.now_us.wrapping_add(delay_us + CONVERSION_US);
        self.conversions += 1;
    }
}

impl<const INPUTS: usize> Converter for Simulated<INPUTS> {
    fn acknowledge() {}

    fn request_interrupt() {}

    fn sample(&self, rank: usize) -> Sample {
        let input = if self.parallel {
            self.order.get(rank).copied().unwrap_or_default()
        } else {
            self.input
        };
        Sample {
            reading: self.reading(input),
            reference: self.reference,
        }
    }

    fn select_address(&mut self, address: u8) {
        self.address = address;
        self.operations.push(Operation::SelectAddress(address));
    }

    fn select_input(&mut self, previous: u8, next: u8) {
        self.enable(previous, false);
        self.enable(next, true);
        self.input = next;
        self.operations.push(Operation::SelectInput(previous, next));
    }

    fn enter_single(&mut self, previous: u8) {
        self.enabled = [false; INPUTS];
        self.enable(0, true);
        self.parallel = false;
        self.input = 0;
        self.address = 0;
        self.operations.push(Operation::EnterSingle(previous));
    }

    fn enter_parallel(&mut self, order: &[u8], address: u8) {
        self.enabled = [false; INPUTS];
        for &input in order {
            self.enable(input, true);
        }
        self.parallel = true;
        self.order = order.to_vec();
        self.address = address;
        self.operations.push(Operation::EnterParallel(address));
    }

    fn sweep_enter_parallel(&mut self, order: Option<&[u8]>) {
        self.parallel = true;
        if let Some(order) = order {
            for &input in order {
                self.enable(input, true);
            }
            self.order = order.to_vec();
        }
        self.operations
            .push(Operation::SweepEnterParallel(order.is_some()));
    }

    fn trigger_after(&mut self, delay_us: u16) {
        self.advance(delay_us);
        self.operations.push(Operation::TriggerAfter(delay_us));
    }

    fn trigger_at(&mut self, ticks: u16) {
        self.advance(ticks);
        self.operations.push(Operation::TriggerAt(ticks));
    }

    fn end_cycle(&mut self, period_us: u16) {
        self.operations.push(Operation::EndCycle(period_us));
    }

    fn recover(&mut self) {
        self.operations.push(Operation::Recover);
    }

    fn micros(&self) -> u16 {
        self.now_us
    }
}

#[derive(Debug)]
pub(crate) struct Bench;

impl Sensing for Bench {
    type Converter = Simulated<6>;
    type Noise = MinMax;
    type Recheck = RecheckFrom<3>;
    type Reference = ZeroLevel<ZERO_LEVEL>;
}

#[derive(Debug)]
pub(super) struct ReferenceBench;

impl Sensing for ReferenceBench {
    type Converter = Simulated<4>;
    type Noise = StandardDeviation;
    type Recheck = ();
    type Reference = MeasuredReference<2482>;
}
