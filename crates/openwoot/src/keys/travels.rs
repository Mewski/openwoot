use embassy_sync::blocking_mutex::raw::RawMutex;

use super::{Key, Shared, normalize_travel, response_curve};
use crate::{
    lighting::{KeyTravel, TRAVEL_FULL},
    scan::{Map, Travel},
    usb::Mode,
};

pub struct Travels<'a, M: RawMutex, const INPUTS: usize, const ADDRESSES: usize> {
    pub travel: &'a Travel<INPUTS, ADDRESSES>,
    pub map: Map<INPUTS, ADDRESSES>,
    pub shared: &'a Shared<M>,
    pub mode: Mode,
}

impl<M: RawMutex, const INPUTS: usize, const ADDRESSES: usize> KeyTravel
    for Travels<'_, M, INPUTS, ADDRESSES>
{
    fn travel(&self, key: Key) -> u16 {
        self.map
            .position(key)
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
