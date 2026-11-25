use critical_section::CriticalSection;

use crate::{pac, register::modify_bits};

pub(super) const APB1EN_I2C0EN: u32 = 1 << 21;
pub(super) const APB1RST_I2C0RST: u32 = 1 << 21;
pub(super) const OWN_ADDRESS: u32 = 0xFF;
pub(super) const CTL0_I2CEN: u32 = 1 << 0;
pub(super) const CTL0_SMBEN: u32 = 1 << 1;
pub(super) const CTL0_START: u32 = 1 << 8;
pub(super) const CTL0_STOP: u32 = 1 << 9;
pub(super) const CTL0_ACKEN: u32 = 1 << 10;
pub(super) const CTL0_SRESET: u32 = 1 << 15;
pub(super) const CTL1_I2CCLK: u32 = 0x3F;
pub(super) const CTL1_ERRIE: u32 = 1 << 8;
pub(super) const CTL1_EVIE: u32 = 1 << 9;
pub(super) const CTL1_DMAON: u32 = 1 << 11;
pub(super) const STAT0_SBSEND: u32 = 1 << 0;
pub(super) const STAT0_ADDSEND: u32 = 1 << 1;
pub(super) const STAT0_BTC: u32 = 1 << 2;
pub(super) const STAT1_MASTER: u32 = 1 << 0;
pub(super) const STAT1_I2CBSY: u32 = 1 << 1;

pub(super) fn registers() -> &'static pac::i2c0::RegisterBlock {
    // SAFETY: an `I2c` owns the I2C0 peripheral; the interrupt handlers only clear its interrupt
    // enable bits and wake the waiting transfer, and `stop_for_reset` only ends the bus traffic
    // right before the microcontroller resets, when no transfer runs on afterwards.
    unsafe { &*pac::I2c0::ptr() }
}

pub(super) fn modify_ctl1(
    _: CriticalSection<'_>,
    i2c: &pac::i2c0::RegisterBlock,
    change: impl FnOnce(u32) -> u32,
) {
    modify_bits(i2c.ctl1(), change);
}

pub(super) const fn with_clock(ctl1: u32, clock_mhz: u32) -> u32 {
    (ctl1 & !CTL1_I2CCLK) | clock_mhz
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_field_changes_without_touching_the_interrupt_and_dma_enables() {
        assert_eq!(with_clock(0, 50), 0x0032);
        assert_eq!(with_clock(CTL1_I2CCLK, 50), 0x0032);
        assert_eq!(
            with_clock(CTL1_ERRIE | CTL1_EVIE | CTL1_DMAON | 0x2A, 50),
            0x0B32
        );
    }

    #[test]
    fn the_clock_field_and_the_enables_are_disjoint() {
        assert_eq!(CTL1_I2CCLK & (CTL1_ERRIE | CTL1_EVIE | CTL1_DMAON), 0);
    }
}
