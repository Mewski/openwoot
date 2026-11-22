use core::hint::spin_loop;

use crate::{
    pac,
    register::{clear_bits, read_bits, set_bits, write_bits},
};

const CTL_IRC16MEN: u32 = 1 << 0;
const CTL_HXTALEN: u32 = 1 << 16;
const CTL_HXTALSTB: u32 = 1 << 17;
const CTL_HXTALBPS: u32 = 1 << 18;
const CTL_CKMEN: u32 = 1 << 19;
const CTL_PLLEN: u32 = 1 << 24;
const CTL_PLLSTB: u32 = 1 << 25;
const CFG0_SCS: u32 = 0b11;
const CFG0_SCS_PLLP: u32 = 0b10;
const CFG0_SCSS_PLLP: u32 = 0b10 << 2;
const CFG0_AHBPSC_SHIFT: u32 = 4;
const CFG0_APB1PSC_SHIFT: u32 = 10;
const CFG0_APB2PSC_SHIFT: u32 = 13;
const CFG1_TIMERSEL: u32 = 1 << 24;
const PLL_RESET: u32 = 0x2400_3010;
const PLL_N_SHIFT: u32 = 6;
const PLL_P_SHIFT: u32 = 16;
const PLL_SOURCE_HXTAL: u32 = 1 << 22;
const PLL_Q_SHIFT: u32 = 24;
const APB1EN_PMUEN: u32 = 1 << 28;
const PMU_CTL_LDOVS: u32 = 0b11 << 14;
const PMU_CTL_HDEN: u32 = 1 << 16;
const PMU_CTL_HDS: u32 = 1 << 17;
const PMU_CS_HDRF: u32 = 1 << 16;
const PMU_CS_HDSRF: u32 = 1 << 17;
const HXTAL_STARTUP_TIMEOUT: u32 = 0xFFFF;
const PRESCALER_STEP_DELAY_CYCLES: u32 = 300_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hertz(pub u32);

impl Hertz {
    #[must_use]
    pub const fn from_mhz(megahertz: u32) -> Self {
        Self(megahertz * 1_000_000)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AhbDivider {
    By1,
    By2,
    By4,
    By8,
    By16,
    By64,
    By128,
    By256,
    By512,
}

impl AhbDivider {
    #[must_use]
    pub const fn divisor(self) -> u32 {
        match self {
            Self::By1 => 1,
            Self::By2 => 2,
            Self::By4 => 4,
            Self::By8 => 8,
            Self::By16 => 16,
            Self::By64 => 64,
            Self::By128 => 128,
            Self::By256 => 256,
            Self::By512 => 512,
        }
    }

    const fn field(self) -> u32 {
        match self {
            Self::By1 => 0b0000,
            Self::By2 => 0b1000,
            Self::By4 => 0b1001,
            Self::By8 => 0b1010,
            Self::By16 => 0b1011,
            Self::By64 => 0b1100,
            Self::By128 => 0b1101,
            Self::By256 => 0b1110,
            Self::By512 => 0b1111,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApbDivider {
    By1,
    By2,
    By4,
    By8,
    By16,
}

impl ApbDivider {
    #[must_use]
    pub const fn divisor(self) -> u32 {
        match self {
            Self::By1 => 1,
            Self::By2 => 2,
            Self::By4 => 4,
            Self::By8 => 8,
            Self::By16 => 16,
        }
    }

    const fn field(self) -> u32 {
        match self {
            Self::By1 => 0b000,
            Self::By2 => 0b100,
            Self::By4 => 0b101,
            Self::By8 => 0b110,
            Self::By16 => 0b111,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PllSystemDivider {
    By2,
    By4,
    By6,
    By8,
}

impl PllSystemDivider {
    #[must_use]
    pub const fn divisor(self) -> u32 {
        match self {
            Self::By2 => 2,
            Self::By4 => 4,
            Self::By6 => 6,
            Self::By8 => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerMultiplier {
    Double,
    Quadruple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pll {
    pub input_divider: u8,
    pub multiplier: u16,
    pub system_divider: PllSystemDivider,
    pub peripheral_divider: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub crystal: Hertz,
    pub pll: Pll,
    pub ahb_divider: AhbDivider,
    pub apb1_divider: ApbDivider,
    pub apb2_divider: ApbDivider,
    pub timer_multiplier: TimerMultiplier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clocks {
    pub pll_input: Hertz,
    pub pll_vco: Hertz,
    pub pll_peripheral: Hertz,
    pub system: Hertz,
    pub ahb: Hertz,
    pub apb1: Hertz,
    pub apb2: Hertz,
    pub apb1_timers: Hertz,
    pub apb2_timers: Hertz,
}

impl Config {
    #[must_use]
    pub const fn clocks(&self) -> Clocks {
        let pll_input = self.crystal.0 / self.pll.input_divider as u32;
        let pll_vco = pll_input * self.pll.multiplier as u32;
        let system = pll_vco / self.pll.system_divider.divisor();
        let ahb = system / self.ahb_divider.divisor();
        Clocks {
            pll_input: Hertz(pll_input),
            pll_vco: Hertz(pll_vco),
            pll_peripheral: Hertz(pll_vco / self.pll.peripheral_divider as u32),
            system: Hertz(system),
            ahb: Hertz(ahb),
            apb1: Hertz(ahb / self.apb1_divider.divisor()),
            apb2: Hertz(ahb / self.apb2_divider.divisor()),
            apb1_timers: Hertz(self.timer_clock(ahb, self.apb1_divider)),
            apb2_timers: Hertz(self.timer_clock(ahb, self.apb2_divider)),
        }
    }

    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "taking the peripheral proves that nothing else configures the clocks"
    )]
    pub fn configure(&self, rcu: pac::Rcu, pmu: &pac::Pmu) -> Clocks {
        reset_to_internal_oscillator(&rcu);
        start_crystal(&rcu);
        set_bits(rcu.apb1en(), APB1EN_PMUEN);
        set_bits(pmu.ctl(), PMU_CTL_LDOVS);
        set_bits(rcu.cfg0(), self.ahb_divider.field() << CFG0_AHBPSC_SHIFT);
        set_bits(rcu.cfg0(), self.apb2_divider.field() << CFG0_APB2PSC_SHIFT);
        set_bits(rcu.cfg0(), self.apb1_divider.field() << CFG0_APB1PSC_SHIFT);
        write_bits(rcu.pll(), self.pll_register());
        set_bits(rcu.ctl(), CTL_PLLEN);
        wait_until(|| read_bits(rcu.ctl()) & CTL_PLLSTB != 0);
        set_bits(pmu.ctl(), PMU_CTL_HDEN);
        wait_until(|| read_bits(pmu.cs()) & PMU_CS_HDRF != 0);
        set_bits(pmu.ctl(), PMU_CTL_HDS);
        wait_until(|| read_bits(pmu.cs()) & PMU_CS_HDSRF != 0);
        clear_bits(rcu.cfg0(), CFG0_SCS);
        set_bits(rcu.cfg0(), CFG0_SCS_PLLP);
        wait_until(|| read_bits(rcu.cfg0()) & CFG0_SCSS_PLLP != 0);
        if self.timer_multiplier == TimerMultiplier::Quadruple {
            set_bits(rcu.cfg1(), CFG1_TIMERSEL);
        }
        self.clocks()
    }

    const fn pll_register(&self) -> u32 {
        self.pll.input_divider as u32
            | (self.pll.multiplier as u32) << PLL_N_SHIFT
            | (self.pll.system_divider.divisor() / 2 - 1) << PLL_P_SHIFT
            | PLL_SOURCE_HXTAL
            | (self.pll.peripheral_divider as u32) << PLL_Q_SHIFT
    }

    const fn timer_clock(&self, ahb: u32, divider: ApbDivider) -> u32 {
        let apb = ahb / divider.divisor();
        match (self.timer_multiplier, divider) {
            (TimerMultiplier::Double, ApbDivider::By1 | ApbDivider::By2)
            | (TimerMultiplier::Quadruple, ApbDivider::By1 | ApbDivider::By2 | ApbDivider::By4) => {
                ahb
            }
            (TimerMultiplier::Double, _) => apb * 2,
            (TimerMultiplier::Quadruple, _) => apb * 4,
        }
    }
}

pub(crate) fn with_registers<Output>(
    change: impl FnOnce(&pac::rcu::RegisterBlock) -> Output,
) -> Output {
    critical_section::with(|_| {
        // SAFETY: `Config::configure` consumed the `Rcu` peripheral; afterwards its registers are
        // changed only here, inside a critical section, so no two read-modify-writes overlap.
        change(unsafe { &*pac::Rcu::ptr() })
    })
}

fn start_crystal(rcu: &pac::Rcu) {
    set_bits(rcu.ctl(), CTL_HXTALEN);
    let stable = (0..HXTAL_STARTUP_TIMEOUT).any(|_| read_bits(rcu.ctl()) & CTL_HXTALSTB != 0);
    if !stable {
        hang();
    }
}

fn reset_to_internal_oscillator(rcu: &pac::Rcu) {
    set_bits(rcu.ctl(), CTL_IRC16MEN);
    set_bits(rcu.cfg0(), AhbDivider::By2.field() << CFG0_AHBPSC_SHIFT);
    cortex_m::asm::delay(PRESCALER_STEP_DELAY_CYCLES);
    set_bits(rcu.cfg0(), AhbDivider::By4.field() << CFG0_AHBPSC_SHIFT);
    cortex_m::asm::delay(PRESCALER_STEP_DELAY_CYCLES);
    clear_bits(rcu.cfg0(), CFG0_SCS);
    clear_bits(rcu.ctl(), CTL_PLLEN | CTL_CKMEN | CTL_HXTALEN);
    clear_bits(rcu.ctl(), CTL_HXTALBPS);
    write_bits(rcu.cfg0(), 0);
    write_bits(rcu.pll(), PLL_RESET);
    write_bits(rcu.int(), 0);
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    while !condition() {
        spin_loop();
    }
}

fn hang() -> ! {
    loop {
        spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_BUILD: Config = Config {
        crystal: Hertz::from_mhz(25),
        pll: Pll {
            input_divider: 25,
            multiplier: 400,
            system_divider: PllSystemDivider::By2,
            peripheral_divider: 9,
        },
        ahb_divider: AhbDivider::By1,
        apb1_divider: ApbDivider::By4,
        apb2_divider: ApbDivider::By2,
        timer_multiplier: TimerMultiplier::Quadruple,
    };

    #[test]
    fn the_c_configuration_gives_the_c_frequencies() {
        let clocks = C_BUILD.clocks();
        assert_eq!(clocks.pll_input, Hertz::from_mhz(1));
        assert_eq!(clocks.pll_vco, Hertz::from_mhz(400));
        assert_eq!(clocks.pll_peripheral, Hertz(44_444_444));
        assert_eq!(clocks.system, Hertz::from_mhz(200));
        assert_eq!(clocks.ahb, Hertz::from_mhz(200));
        assert_eq!(clocks.apb1, Hertz::from_mhz(50));
        assert_eq!(clocks.apb2, Hertz::from_mhz(100));
        assert_eq!(clocks.apb1_timers, Hertz::from_mhz(200));
        assert_eq!(clocks.apb2_timers, Hertz::from_mhz(200));
    }

    #[test]
    fn the_c_configuration_gives_the_c_register_values() {
        assert_eq!(C_BUILD.pll_register(), 0x0940_6419);
        assert_eq!(C_BUILD.ahb_divider.field() << CFG0_AHBPSC_SHIFT, 0);
        assert_eq!(C_BUILD.apb2_divider.field() << CFG0_APB2PSC_SHIFT, 0x8000);
        assert_eq!(C_BUILD.apb1_divider.field() << CFG0_APB1PSC_SHIFT, 0x1400);
        assert_eq!(AhbDivider::By2.field() << CFG0_AHBPSC_SHIFT, 0x80);
        assert_eq!(AhbDivider::By4.field() << CFG0_AHBPSC_SHIFT, 0x90);
    }

    #[test]
    fn timers_follow_the_timer_clock_selection() {
        let doubled = Config {
            timer_multiplier: TimerMultiplier::Double,
            ..C_BUILD
        };
        assert_eq!(doubled.clocks().apb1_timers, Hertz::from_mhz(100));
        assert_eq!(doubled.clocks().apb2_timers, Hertz::from_mhz(200));
        let slow_apb1 = Config {
            apb1_divider: ApbDivider::By8,
            ..C_BUILD
        };
        assert_eq!(slow_apb1.clocks().apb1_timers, Hertz::from_mhz(100));
    }
}
