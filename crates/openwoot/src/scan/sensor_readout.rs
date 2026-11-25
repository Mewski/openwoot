use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use openwoot_protocol::sensors::{Reading, Values};

use super::{Scanner, Sensing, Sensor, Travel};
use crate::keys::Key;

pub struct SensorReadout {
    raw_scanning: AtomicBool,
    request: Signal<CriticalSectionRawMutex, Reading>,
    values: Mutex<CriticalSectionRawMutex, RefCell<Values>>,
    published: Signal<CriticalSectionRawMutex, Reading>,
}

impl SensorReadout {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            raw_scanning: AtomicBool::new(false),
            request: Signal::new(),
            values: Mutex::new(RefCell::new(Values::new())),
            published: Signal::new(),
        }
    }

    pub fn raw_scanning(&self) -> bool {
        self.raw_scanning.load(Ordering::Relaxed)
    }

    pub fn serve<
        const ROWS: usize,
        const COLUMNS: usize,
        const INPUTS: usize,
        const ADDRESSES: usize,
    >(
        &self,
        scanner: &mut Scanner<impl Sensing, INPUTS, ADDRESSES>,
        travel: &Travel<INPUTS, ADDRESSES>,
    ) {
        scanner.set_raw_scanning(self.raw_scanning());
        let Some(reading) = self.request.try_take() else {
            return;
        };
        self.values.lock(|cell| {
            let Ok(mut values) = cell.try_borrow_mut() else {
                return;
            };
            values.clear();
            if values.resize(ROWS * COLUMNS, None).is_err() {
                return;
            }
            let mut store = |key: Key, value: Option<u16>| {
                if let Some(slot) = values.get_mut(key.index(COLUMNS)) {
                    *slot = value;
                }
            };
            if reading == Reading::ScanOrder {
                for (order, (_, key)) in (0..=u16::MAX).zip(scanner.conversion_order()) {
                    store(key, Some(order));
                }
                return;
            }
            for (position, key) in scanner.map().iter() {
                let sensor = scanner.sensor(position);
                let value = match reading {
                    Reading::Travel => Some(travel.get(position)),
                    Reading::Value => sensor.map(Sensor::value),
                    Reading::RestLevel => sensor.map(Sensor::rest_level),
                    Reading::SingleRestLevel => sensor.map(|sensor| sensor.single().rest_level),
                    Reading::ParallelRestLevel => sensor.map(|sensor| sensor.parallel().rest_level),
                    Reading::BottomLevel => sensor.map(Sensor::bottom_level),
                    Reading::AboveRest => sensor.map(|sensor| u16::from(sensor.is_above_rest())),
                    Reading::ScanOrder => None,
                };
                store(key, value);
            }
        });
        self.published.signal(reading);
    }

    pub(crate) fn set_raw_scanning(&self, enabled: bool) {
        self.raw_scanning.store(enabled, Ordering::Relaxed);
    }

    pub(crate) async fn read(&self, reading: Reading, values: &mut Values) {
        self.published.reset();
        self.request.signal(reading);
        while self.published.wait().await != reading {}
        self.values.lock(|cell| {
            if let Ok(published) = cell.try_borrow() {
                values.clone_from(&published);
            }
        });
    }
}

impl Default for SensorReadout {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for SensorReadout {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SensorReadout")
            .field("raw_scanning", &self.raw_scanning())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use core::{
        pin::pin,
        task::{Context, Waker},
    };

    use super::*;

    #[test]
    fn a_publication_of_another_reading_does_not_answer_a_read() {
        let readout = SensorReadout::new();
        let mut values = Values::new();
        let mut read = pin!(readout.read(Reading::Travel, &mut values));
        let mut context = Context::from_waker(Waker::noop());
        assert!(read.as_mut().poll(&mut context).is_pending());
        readout.published.signal(Reading::RestLevel);
        assert!(read.as_mut().poll(&mut context).is_pending());
        readout.published.signal(Reading::Travel);
        assert!(read.as_mut().poll(&mut context).is_ready());
    }
}
