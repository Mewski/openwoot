use super::{CYCLE_PERIOD_US, DELAY_SAME_INPUT_US, Outcome, Scanner};
use crate::scan::{Converter, Position, Recheck, Reference, Sensing, Travel};

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    pub(super) fn parallel_samples(&mut self, travel: &Travel<INPUTS, ADDRESSES>) -> Outcome {
        let address = self.address;
        if self.recheck.is_second_conversion() {
            for rank in 0..INPUTS {
                let input = self.order.get(rank).copied().unwrap_or_default();
                let position = Position::new(input, address);
                if !self.recheck.is_deferred(input) || self.map.key(position).is_none() {
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
                let Some(sensor) = self.sensors.get_mut(position) else {
                    continue;
                };
                let value = sensor.filter(magnitude);
                if self.recheck.defer(input, value, sensor.filtered()) {
                    continue;
                }
                sensor.set_filtered(value);
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
            self.cycle_finished = true;
            return Outcome::Finished;
        }
        Outcome::Running
    }

    fn store_parallel(&mut self, rank: usize, position: Position, magnitude: u16) {
        if let Some(sensor) = self.sensors.get_mut(position) {
            let value = sensor.filter(magnitude);
            sensor.set_filtered(value);
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
        let Some(sensor) = self.sensors.get_mut(position) else {
            return;
        };
        let update = sensor.update(&self.converter, &self.curves, context, magnitude, value);
        self.finish_key(position, update, travel);
    }
}

#[inline(always)]
fn set<const INPUTS: usize>(values: &mut [u16; INPUTS], rank: usize, value: u16) {
    if let Some(slot) = values.get_mut(rank) {
        *slot = value;
    }
}
