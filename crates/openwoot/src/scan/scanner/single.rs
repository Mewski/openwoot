use super::{CYCLE_PERIOD_US, DELAY_INPUT_US, DELAY_SAME_INPUT_US, Outcome, Scanner};
use crate::scan::{Converter, Grid, Map, Position, Recheck, Reference, Sensing, Travel};

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    pub(super) fn single_sample(&mut self, travel: &Travel<INPUTS, ADDRESSES>) -> Outcome {
        let sample = self.converter.sample(0);
        let converted = self.pending;
        if converted.is_some_and(|position| position.index() == 0) {
            self.reference.latch(sample);
        }
        let magnitude = self.reference.magnitude(sample);
        let context = self.context();
        let mut sensor = converted.and_then(|position| self.sensors.get_mut(position));
        let mut value = 0;
        if let Some(sensor) = sensor.as_deref_mut() {
            value = sensor.filter(magnitude);
            if self
                .recheck
                .reconvert_single(self.input, value, sensor.filtered())
            {
                self.converter.trigger_after(DELAY_SAME_INPUT_US);
                return Outcome::Running;
            }
            sensor.set_filtered(value);
        }

        let current = Position::new(self.input, self.address);
        let next = self
            .successors
            .get(current)
            .copied()
            .unwrap_or(Position::new(0, 0));
        let cycle_end = next.index() <= current.index();
        self.address = next.address();
        self.converter.select_address(next.address());
        let previous = self.input;
        if previous == next.input() {
            if !cycle_end {
                self.converter.trigger_after(DELAY_SAME_INPUT_US);
            }
        } else {
            self.converter.select_input(previous, next.input());
            self.input = next.input();
            if !cycle_end {
                self.converter.trigger_after(DELAY_INPUT_US);
            }
        }

        let update = sensor
            .map(|sensor| sensor.update(&self.converter, &self.curves, context, magnitude, value));
        self.pending = Some(next);
        if let (Some(position), Some(update)) = (converted, update) {
            self.finish_key(position, update, travel);
        }
        if cycle_end {
            self.converter.end_cycle(CYCLE_PERIOD_US);
            self.cycle_finished = true;
            return Outcome::Finished;
        }
        Outcome::Running
    }
}

pub(super) fn successors<const INPUTS: usize, const ADDRESSES: usize>(
    map: &Map<INPUTS, ADDRESSES>,
) -> Grid<Position, INPUTS> {
    Grid::from_fn(|start| {
        let mut input = usize::from(start.input());
        let mut address = usize::from(start.address());
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
}
