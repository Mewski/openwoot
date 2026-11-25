use super::Scanner;
use crate::scan::{Advance, Sensing, Sensor};

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    pub(super) fn restart_batch(&mut self) {
        self.for_each_batch_sensor(Sensor::restart_noise);
    }

    pub(super) fn poll_batch(&mut self) -> bool {
        let mut sampling = false;
        self.for_each_batch_sensor(|sensor| sampling |= sensor.poll_recalibration());
        sampling
    }

    pub(super) fn advance_batch(&mut self, now_ms: u32) {
        match self
            .recalibration
            .advance(self.mode, now_ms, INPUTS, ADDRESSES)
        {
            Advance::NextBatch => self.restart_batch(),
            Advance::PassComplete => {
                for sensor in self.sensors.iter_mut() {
                    sensor.finish_pass();
                }
            }
        }
    }

    pub(super) fn stop_sampling(&mut self) {
        if self.recalibration.stop() {
            for sensor in self.sensors.iter_mut() {
                sensor.stop_recalibrating();
            }
        }
    }

    pub(super) fn note_activity(&mut self) {
        self.recalibration.hold(self.now_ms);
        self.stop_sampling();
    }

    fn for_each_batch_sensor(&mut self, mut visit: impl FnMut(&mut Sensor<S::Noise>)) {
        let map = &self.map;
        let sensors = &mut *self.sensors;
        self.recalibration.batching().for_each_member(
            self.recalibration.batch(),
            INPUTS,
            ADDRESSES,
            |position| {
                if map.key(position).is_some()
                    && let Some(sensor) = sensors.get_mut(position)
                {
                    visit(sensor);
                }
            },
        );
    }
}
