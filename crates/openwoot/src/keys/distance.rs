use openwoot_protocol::keys::Distance;

use crate::lighting::TRAVEL_FULL;

#[must_use]
pub fn travel_of(distance: Distance) -> u16 {
    let travel = u32::from(TRAVEL_FULL) * u32::from(distance.fraction()) / u32::from(u16::MAX);
    u16::try_from(travel).unwrap_or(TRAVEL_FULL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_fraction_is_the_full_travel() {
        assert_eq!(travel_of(Distance::FULL_TRAVEL), TRAVEL_FULL);
        assert_eq!(travel_of(Distance::from_fraction(0x8000)), 2047);
        assert_eq!(travel_of(Distance::from_fraction(257 * 63)), 1011);
    }
}
