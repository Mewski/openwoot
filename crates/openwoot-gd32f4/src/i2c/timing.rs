use crate::rcu::Hertz;

pub(super) const BUS_HZ: u32 = 400_000;

const CKCFG_DTCY: u32 = 1 << 14;
const CKCFG_FAST: u32 = 1 << 15;
const CKCFG_CLKC: u32 = 0xFFF;
const CLOCK_MAX_MHZ: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Timing {
    pub(super) clock_mhz: u32,
    pub(super) rise_time: u32,
    pub(super) clock_configuration: u32,
}

pub(super) const fn timing(apb1: Hertz) -> Timing {
    let mut clock_mhz = apb1.0 / 1_000_000;
    if clock_mhz > CLOCK_MAX_MHZ {
        clock_mhz = CLOCK_MAX_MHZ;
    }
    let mut divider = apb1.0 / (BUS_HZ * 25);
    if divider & CKCFG_CLKC == 0 {
        divider |= 1;
    }
    Timing {
        clock_mhz,
        rise_time: clock_mhz * 300 / 1000 + 1,
        clock_configuration: CKCFG_FAST | CKCFG_DTCY | (divider & CKCFG_CLKC),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_mode_timing_matches_the_reference_set_up() {
        assert_eq!(
            timing(Hertz::from_mhz(50)),
            Timing {
                clock_mhz: 50,
                rise_time: 16,
                clock_configuration: 0xC005,
            }
        );
        assert_eq!(timing(Hertz::from_mhz(100)).clock_mhz, 60);
    }
}
