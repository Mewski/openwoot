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

pub struct SensorReadout {
    raw_scanning: AtomicBool,
    request: Signal<CriticalSectionRawMutex, Reading>,
    values: Mutex<CriticalSectionRawMutex, RefCell<Values>>,
    published: Signal<CriticalSectionRawMutex, ()>,
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
            for (position, key) in scanner.map().iter() {
                let sensor = scanner.sensor(position);
                let value = match reading {
                    Reading::Travel => Some(travel.get(position)),
                    Reading::Value => sensor.map(Sensor::value),
                    Reading::RestLevel => sensor.map(Sensor::rest_level),
                    Reading::BottomLevel => sensor.map(Sensor::bottom_level),
                };
                if let Some(slot) = values.get_mut(key.index(COLUMNS)) {
                    *slot = value;
                }
            }
        });
        self.published.signal(());
    }

    pub(crate) fn set_raw_scanning(&self, enabled: bool) {
        self.raw_scanning.store(enabled, Ordering::Relaxed);
    }

    pub(crate) async fn read(&self, reading: Reading, values: &mut Values) {
        self.published.reset();
        self.request.signal(reading);
        self.published.wait().await;
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
