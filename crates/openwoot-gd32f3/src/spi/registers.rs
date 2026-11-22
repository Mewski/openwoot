pub(super) const CTL0_SPIEN: u32 = 1 << 6;
pub(super) const CTL1_DMAREN: u32 = 1 << 0;
pub(super) const CTL1_DMATEN: u32 = 1 << 1;
pub(super) const STAT_RBNE: u32 = 1 << 0;
pub(super) const STAT_TRANS: u32 = 1 << 7;
pub(super) const I2SCTL_I2SSEL: u32 = 1 << 11;

const CTL0_MASTER: u32 = 1 << 2;
const CTL0_SWNSS: u32 = 1 << 8;
const CTL0_SWNSSEN: u32 = 1 << 9;
const CTL0_KEPT: u32 = 0x3040;

pub(super) const fn control(ctl0: u32) -> u32 {
    (ctl0 & CTL0_KEPT) | CTL0_MASTER | CTL0_SWNSS | CTL0_SWNSSEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bus_is_a_software_selected_mode_0_master_at_half_the_bus_clock() {
        assert_eq!(control(0), 0x0304);
        assert_eq!(control(0xFFFF), 0x3344);
        assert_eq!(control(0) | CTL0_SPIEN, 0x0344);
    }
}
