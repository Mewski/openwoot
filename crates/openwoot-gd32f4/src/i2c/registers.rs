use crate::pac;

pub(super) const APB1EN_I2C0EN: u32 = 1 << 21;
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
    // enable bits and wake the waiting transfer.
    unsafe { &*pac::I2c0::ptr() }
}
