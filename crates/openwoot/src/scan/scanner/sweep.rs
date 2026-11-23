use super::{DELAY_INPUT_US, DELAY_SAME_INPUT_US, Outcome, Scanner, Trigger};
use crate::scan::{Converter, Mode, Position, Reference, Sensing};

const SWEEP_SAMPLES: u8 = 100;
const DELAY_FIRST_US: u16 = 254;
const DELAY_ADDRESS_US: u16 = 149;
const DELAY_SWEEP_REPEAT_US: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Sweep {
    samples_left: u8,
    mode: Mode,
}

impl Sweep {
    const fn is_parallel(self) -> bool {
        matches!(self.mode, Mode::Parallel)
    }
}

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    pub fn start_sweep(&mut self) {
        if self.mode_is_parallel() || self.sweep.is_some_and(Sweep::is_parallel) {
            self.set_mode(Mode::Single);
        }
        self.applied = None;
        self.cycle_finished = true;
        for sensor in self.sensors.iter_mut() {
            sensor.reset_for_sweep();
        }
        self.sweep = Some(Sweep {
            samples_left: SWEEP_SAMPLES,
            mode: Mode::Single,
        });
        self.sweep_done = false;
        self.converter.select_input(self.input, 0);
        self.converter.select_address(0);
        self.input = 0;
        self.address = 0;
        self.trigger = Some(Trigger::After(DELAY_FIRST_US));
    }

    #[inline(never)]
    pub(super) fn sweep_step(&mut self, sweep: Sweep) -> Outcome {
        let samples_left = self.track_sweep_samples(sweep).wrapping_sub(1);
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
            self.converter
                .trigger_after(if !sweep.is_parallel() && unkeyed {
                    DELAY_SAME_INPUT_US
                } else {
                    DELAY_ADDRESS_US
                });
            return Outcome::Running;
        }
        self.address = 0;
        self.converter.select_address(0);
        if sweep.is_parallel() {
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
            next.mode = Mode::Parallel;
            self.converter.sweep_enter_parallel(Some(&self.order));
        }
        self.sweep = Some(next);
        self.converter.trigger_after(DELAY_INPUT_US);
        Outcome::Running
    }

    fn track_sweep_samples(&mut self, sweep: Sweep) -> u8 {
        let ranks = if sweep.is_parallel() { INPUTS } else { 1 };
        for rank in 0..ranks {
            let input = if sweep.is_parallel() {
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
            let Some(sensor) = self.sensors.get_mut(position) else {
                continue;
            };
            sensor.track_sweep(magnitude);
            if sweep.samples_left == 1 {
                sensor.finish_sweep(sweep.mode);
            }
        }
        sweep.samples_left
    }
}
