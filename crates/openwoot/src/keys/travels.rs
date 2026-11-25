use embassy_sync::blocking_mutex::raw::RawMutex;

use super::{Key, ProfileSource, Shared, normalize_travel, response_curve};
use crate::{
    lighting::{KeyTravel, TRAVEL_FULL},
    scan::{ADDRESSES_PER_INPUT, Map, Position, Travel},
    usb::Mode,
};

const CODED_ROWS: usize = 8;
const CODED_COLUMNS: usize = 32;
const UNSENSED: u8 = u8::MAX;

pub struct Travels<'a, M: RawMutex, const INPUTS: usize, const ADDRESSES: usize> {
    travel: &'a Travel<INPUTS, ADDRESSES>,
    sensors: [[u8; CODED_COLUMNS]; CODED_ROWS],
    shared: &'a Shared<M>,
    mode: Mode,
}

impl<'a, M: RawMutex, const INPUTS: usize, const ADDRESSES: usize>
    Travels<'a, M, INPUTS, ADDRESSES>
{
    #[must_use]
    pub fn new(
        travel: &'a Travel<INPUTS, ADDRESSES>,
        map: &Map<INPUTS, ADDRESSES>,
        shared: &'a Shared<M>,
        mode: Mode,
    ) -> Self {
        const { assert!(INPUTS * ADDRESSES_PER_INPUT <= UNSENSED as usize) };
        let mut sensors = [[UNSENSED; CODED_COLUMNS]; CODED_ROWS];
        for (position, key) in map.iter() {
            if let Some(sensor) = sensors
                .get_mut(key.row())
                .and_then(|row| row.get_mut(key.column()))
                && *sensor == UNSENSED
            {
                *sensor = position.index();
            }
        }
        Self {
            travel,
            sensors,
            shared,
            mode,
        }
    }

    fn position(&self, key: Key) -> Option<Position> {
        let index = *self.sensors.get(key.row())?.get(key.column())?;
        (index != UNSENSED).then(|| Position::from_index(index))
    }
}

impl<M: RawMutex, const INPUTS: usize, const ADDRESSES: usize> KeyTravel
    for Travels<'_, M, INPUTS, ADDRESSES>
{
    fn travel(&self, key: Key) -> u16 {
        self.position(key)
            .map_or(0, |position| self.travel.get(position))
    }

    fn gamepad_mode(&self) -> bool {
        self.mode != Mode::NoGamepad
    }

    fn response_curve(&self, travel: u16) -> f32 {
        let settings = self.shared.read(|profile| profile.gamepad_settings);
        let settings = settings.as_ref();
        settings.map_or_else(
            || f32::from(travel) / f32::from(TRAVEL_FULL),
            |_| response_curve(normalize_travel(u32::from(travel), settings), settings),
        )
    }
}

impl<M: RawMutex, const INPUTS: usize, const ADDRESSES: usize> core::fmt::Debug
    for Travels<'_, M, INPUTS, ADDRESSES>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Travels")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;

    use super::*;

    const MAP: Map<2, 3> = Map::from_codes([
        [
            0x44, 0xFF, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ],
        [
            0x44, 0x01, 0xA2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ],
    ]);

    #[test]
    fn every_key_reads_the_sensor_the_map_finds_first_for_it() {
        let travel = Travel::new();
        let shared = Shared::<NoopRawMutex>::new();
        let travels = Travels::new(&travel, &MAP, &shared, Mode::NoGamepad);
        for row in 0..=u8::MAX {
            for column in 0..=u8::MAX {
                let key = Key::new(row, column);
                assert_eq!(travels.position(key), MAP.position(key), "{key:?}");
            }
        }
        assert_eq!(travels.position(Key::new(2, 4)), Some(Position::new(0, 0)));
    }
}
