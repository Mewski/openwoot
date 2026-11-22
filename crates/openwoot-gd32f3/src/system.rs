use crate::{
    pac, rcu,
    register::{clear_bits, read_bits, set_bits, write_bits},
};

const STAY_IN_UPDATE_MODE: u16 = 0x504;
const FORMAT_REQUEST: u16 = 0x420;
const PMU_CTL_BKPWEN: u32 = 1 << 8;
const BKP_TPCS_TER: u16 = 1 << 0;
const APB1EN_BKPIEN: u32 = 1 << 27;

pub fn reset() -> ! {
    cortex_m::peripheral::SCB::sys_reset()
}

#[derive(Debug)]
pub struct Bootloader {
    pmu: pac::Pmu,
    bkp: pac::Bkp,
}

impl Bootloader {
    #[must_use]
    pub const fn new(pmu: pac::Pmu, bkp: pac::Bkp) -> Self {
        Self { pmu, bkp }
    }

    pub fn enter_update_mode(&mut self, option: u8) -> ! {
        self.open_backup();
        self.write_backup(|bkp| {
            write_bits(bkp.data0(), STAY_IN_UPDATE_MODE);
            write_bits(bkp.data2(), u16::from(option));
        });
        reset()
    }

    pub fn restart(&mut self, option: u8) -> ! {
        if option != 0 {
            self.open_backup();
            self.write_backup(|bkp| write_bits(bkp.data2(), u16::from(option)));
        }
        reset()
    }

    pub fn take_format_request(&mut self) -> bool {
        self.open_backup();
        let requested = read_bits(self.bkp.data1()) == FORMAT_REQUEST;
        if requested {
            self.write_backup(|bkp| write_bits(bkp.data1(), 0));
        }
        rcu::with_registers(|rcu| clear_bits(rcu.apb1en(), APB1EN_BKPIEN));
        requested
    }

    fn open_backup(&self) {
        rcu::with_registers(|rcu| set_bits(rcu.apb1en(), APB1EN_BKPIEN));
        set_bits(self.bkp.tpcs(), BKP_TPCS_TER);
    }

    fn write_backup(&self, write: impl FnOnce(&pac::bkp::RegisterBlock)) {
        set_bits(self.pmu.ctl(), PMU_CTL_BKPWEN);
        write(&self.bkp);
        clear_bits(self.pmu.ctl(), PMU_CTL_BKPWEN);
    }
}
