use openwoot::protocol::UpdateMode;

use crate::{
    pac,
    register::{clear_bits, read_bits, set_bits, write_bits},
};

const STAY_IN_UPDATE_MODE: u32 = 0x504;
const FORMAT_REQUEST: u32 = 0x420;
const PMU_CTL_BKPWEN: u32 = 1 << 8;

pub fn reset() -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

#[derive(Debug)]
pub struct Bootloader {
    pmu: pac::Pmu,
    rtc: pac::Rtc,
}

impl Bootloader {
    #[must_use]
    pub const fn new(pmu: pac::Pmu, rtc: pac::Rtc) -> Self {
        Self { pmu, rtc }
    }

    pub fn restart(&mut self, option: u8) -> ! {
        if option != 0 {
            self.write_backup(|rtc| write_bits(rtc.bkp2(), u32::from(option)));
        }
        reset()
    }

    pub fn take_format_request(&mut self) -> bool {
        if read_bits(self.rtc.bkp1()) != FORMAT_REQUEST {
            return false;
        }
        self.write_backup(|rtc| write_bits(rtc.bkp1(), 0));
        true
    }

    fn write_backup(&self, write: impl FnOnce(&pac::rtc::RegisterBlock)) {
        set_bits(self.pmu.ctl(), PMU_CTL_BKPWEN);
        write(&self.rtc);
        clear_bits(self.pmu.ctl(), PMU_CTL_BKPWEN);
    }
}

impl UpdateMode for Bootloader {
    fn enter_update_mode(&mut self, option: u8) {
        self.write_backup(|rtc| {
            write_bits(rtc.bkp0(), STAY_IN_UPDATE_MODE);
            write_bits(rtc.bkp2(), u32::from(option));
        });
        reset()
    }
}
