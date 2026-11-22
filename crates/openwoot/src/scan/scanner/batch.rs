use super::Scanner;
use crate::scan::{Advance, Position, Sensing};

impl<S: Sensing, const INPUTS: usize, const ADDRESSES: usize> Scanner<S, INPUTS, ADDRESSES> {
    pub(super) fn restart_batch(&mut self) {
        for position in self.batch_positions() {
            if let Some(sensor) = self.sensors.get_mut(position) {
                sensor.restart_noise();
            }
        }
    }

    pub(super) fn poll_batch(&mut self) -> bool {
        let mut sampling = false;
        for position in self.batch_positions() {
            if let Some(sensor) = self.sensors.get_mut(position) {
                sampling |= sensor.poll_recalibration();
            }
        }
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
                    sensor.stop_recalibrating();
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

    fn batch_positions(&self) -> impl Iterator<Item = Position> + use<S, INPUTS, ADDRESSES> {
        let map = self.map;
        self.recalibration
            .batching()
            .members(self.recalibration.batch(), INPUTS, ADDRESSES)
            .filter(move |&position| map.key(position).is_some())
    }
}
