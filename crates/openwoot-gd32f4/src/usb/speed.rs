use core::sync::atomic::{AtomicU8, Ordering};

use embassy_usb_driver::Speed;

const NOT_NEGOTIATED: u8 = 0;

static NEGOTIATED: AtomicU8 = AtomicU8::new(NOT_NEGOTIATED);

#[must_use]
pub fn negotiated_speed() -> Option<Speed> {
    decode(NEGOTIATED.load(Ordering::Acquire))
}

pub(super) fn record(enumeration_speed: u8) {
    NEGOTIATED.store(
        encode(enumerated_speed(enumeration_speed)),
        Ordering::Release,
    );
}

const fn enumerated_speed(enumeration_speed: u8) -> Speed {
    match enumeration_speed {
        0b00 => Speed::High,
        0b10 => Speed::Low,
        _ => Speed::Full,
    }
}

const fn encode(speed: Speed) -> u8 {
    match speed {
        Speed::Low => 1,
        Speed::Full => 2,
        Speed::High => 3,
    }
}

const fn decode(encoded: u8) -> Option<Speed> {
    match encoded {
        1 => Some(Speed::Low),
        2 => Some(Speed::Full),
        3 => Some(Speed::High),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enumerated_speed_follows_the_device_status_encoding() {
        assert_eq!(enumerated_speed(0b00), Speed::High);
        assert_eq!(enumerated_speed(0b01), Speed::Full);
        assert_eq!(enumerated_speed(0b10), Speed::Low);
        assert_eq!(enumerated_speed(0b11), Speed::Full);
    }

    #[test]
    fn no_speed_is_reported_before_the_first_reset() {
        assert_eq!(negotiated_speed(), None);
        for speed in [Speed::Low, Speed::Full, Speed::High] {
            assert_eq!(decode(encode(speed)), Some(speed));
        }
    }
}
