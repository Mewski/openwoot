use core::hint::spin_loop;

use crate::{
    pac,
    register::{clear_bits, read_bits, set_bits, write_bits},
};

const CTL_IRC8MEN: u32 = 1 << 0;
const CTL_IRC8MSTB: u32 = 1 << 1;
const CTL_HXTALEN: u32 = 1 << 16;
const CTL_HXTALBPS: u32 = 1 << 18;
const CTL_CKMEN: u32 = 1 << 19;
const CTL_PLLEN: u32 = 1 << 24;
const CTL_PLLSTB: u32 = 1 << 25;
const CFG0_SCS: u32 = 0b11;
const CFG0_SCS_PLL: u32 = 0b10;
const CFG0_SCSS_PLL: u32 = 0b10 << 2;
const CFG0_AHBPSC_SHIFT: u32 = 4;
const CFG0_APB1PSC_SHIFT: u32 = 8;
const CFG0_APB2PSC_SHIFT: u32 = 11;
const CFG0_PLLMF_LOW_SHIFT: u32 = 18;
const CFG0_PLLMF_LOW: u32 = 0b1111 << CFG0_PLLMF_LOW_SHIFT;
const CFG0_PLLMF_4: u32 = 1 << 27;
const CFG0_PLLMF_5: u32 = 1 << 30;
const INT_DISABLE_AND_CLEAR: u32 = 0x009F_0000;
const APB1EN_PMUEN: u32 = 1 << 28;
const PMU_CTL_LDOVS: u32 = 0b11 << 14;
const PMU_CTL_HDEN: u32 = 1 << 16;
const PMU_CTL_HDS: u32 = 1 << 17;
const PMU_CS_HDRF: u32 = 1 << 16;
const PMU_CS_HDSRF: u32 = 1 << 17;
const IRC8M_STARTUP_TIMEOUT: u32 = 0x500;
const PRESCALER_STEP_DELAY_CYCLES: u32 = 120_000;
const PLL_INPUT: Hertz = Hertz::from_mhz(4);

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
pub struct PllMultiplier(u8);

impl PllMultiplier {
    #[must_use]
    pub const fn new<const MULTIPLIER: u8>() -> Self {
        const { assert!(MULTIPLIER >= 2 && MULTIPLIER <= 63) };
        Self(MULTIPLIER)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    const fn field(self) -> u32 {
        let multiplier = self.0 as u32;
        match multiplier {
            2..=16 => (multiplier - 2) << CFG0_PLLMF_LOW_SHIFT,
            17..=32 => CFG0_PLLMF_4 | (multiplier - 17) << CFG0_PLLMF_LOW_SHIFT,
            33..=48 => CFG0_PLLMF_5 | (multiplier - 33) << CFG0_PLLMF_LOW_SHIFT,
            _ => CFG0_PLLMF_4 | CFG0_PLLMF_5 | (multiplier - 49) << CFG0_PLLMF_LOW_SHIFT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockTree {
    pub pll_multiplier: PllMultiplier,
    pub ahb_divider: AhbDivider,
    pub apb1_divider: ApbDivider,
    pub apb2_divider: ApbDivider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clocks {
    pub pll_input: Hertz,
    pub system: Hertz,
    pub ahb: Hertz,
    pub apb1: Hertz,
    pub apb2: Hertz,
    pub apb1_timers: Hertz,
    pub apb2_timers: Hertz,
}

impl ClockTree {
    #[must_use]
    pub const fn clocks(&self) -> Clocks {
        let system = PLL_INPUT.0 * self.pll_multiplier.get() as u32;
        let ahb = system / self.ahb_divider.divisor();
        Clocks {
            pll_input: PLL_INPUT,
            system: Hertz(system),
            ahb: Hertz(ahb),
            apb1: Hertz(ahb / self.apb1_divider.divisor()),
            apb2: Hertz(ahb / self.apb2_divider.divisor()),
            apb1_timers: Hertz(timer_clock(ahb, self.apb1_divider)),
            apb2_timers: Hertz(timer_clock(ahb, self.apb2_divider)),
        }
    }

    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "taking the peripheral proves that nothing else configures the clocks"
    )]
    pub fn configure(&self, rcu: pac::Rcu, pmu: &pac::Pmu) -> Clocks {
        reset_to_internal_oscillator(&rcu);
        start_internal_oscillator(&rcu);
        set_bits(rcu.apb1en(), APB1EN_PMUEN);
        set_bits(pmu.ctl(), PMU_CTL_LDOVS);
        set_bits(rcu.cfg0(), self.ahb_divider.field() << CFG0_AHBPSC_SHIFT);
        set_bits(rcu.cfg0(), self.apb2_divider.field() << CFG0_APB2PSC_SHIFT);
        set_bits(rcu.cfg0(), self.apb1_divider.field() << CFG0_APB1PSC_SHIFT);
        clear_bits(rcu.cfg0(), CFG0_PLLMF_LOW | CFG0_PLLMF_4 | CFG0_PLLMF_5);
        set_bits(rcu.cfg0(), self.pll_multiplier.field());
        set_bits(rcu.ctl(), CTL_PLLEN);
        wait_until(|| read_bits(rcu.ctl()) & CTL_PLLSTB != 0);
        set_bits(pmu.ctl(), PMU_CTL_HDEN);
        wait_until(|| read_bits(pmu.cs()) & PMU_CS_HDRF != 0);
        set_bits(pmu.ctl(), PMU_CTL_HDS);
        wait_until(|| read_bits(pmu.cs()) & PMU_CS_HDSRF != 0);
        clear_bits(rcu.cfg0(), CFG0_SCS);
        set_bits(rcu.cfg0(), CFG0_SCS_PLL);
        wait_until(|| read_bits(rcu.cfg0()) & CFG0_SCSS_PLL != 0);
        self.clocks()
    }
}

pub(crate) fn with_registers<Output>(
    change: impl FnOnce(&pac::rcu::RegisterBlock) -> Output,
) -> Output {
    critical_section::with(|_| {
        // SAFETY: `ClockTree::configure` consumed the `Rcu` peripheral; afterwards its registers
        // are changed only here, inside a critical section, so no two read-modify-writes overlap.
        change(unsafe { &*pac::Rcu::ptr() })
    })
}

const fn timer_clock(ahb: u32, divider: ApbDivider) -> u32 {
    let apb = ahb / divider.divisor();
    match divider {
        ApbDivider::By1 => apb,
        _ => apb * 2,
    }
}

fn reset_to_internal_oscillator(rcu: &pac::Rcu) {
    set_bits(rcu.ctl(), CTL_IRC8MEN);
    set_bits(rcu.cfg0(), AhbDivider::By2.field() << CFG0_AHBPSC_SHIFT);
    cortex_m::asm::delay(PRESCALER_STEP_DELAY_CYCLES);
    clear_bits(rcu.cfg0(), CFG0_SCS);
    clear_bits(rcu.ctl(), CTL_PLLEN | CTL_CKMEN | CTL_HXTALEN);
    write_bits(rcu.int(), INT_DISABLE_AND_CLEAR);
    clear_bits(rcu.ctl(), CTL_HXTALBPS);
    write_bits(rcu.cfg0(), 0);
    write_bits(rcu.cfg1(), 0);
}

fn start_internal_oscillator(rcu: &pac::Rcu) {
    set_bits(rcu.ctl(), CTL_IRC8MEN);
    let stable = (0..IRC8M_STARTUP_TIMEOUT).any(|_| read_bits(rcu.ctl()) & CTL_IRC8MSTB != 0);
    if !stable {
        hang();
    }
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

    const C_BUILD: ClockTree = ClockTree {
        pll_multiplier: PllMultiplier::new::<30>(),
        ahb_divider: AhbDivider::By1,
        apb1_divider: ApbDivider::By2,
        apb2_divider: ApbDivider::By1,
    };

    #[test]
    fn the_c_configuration_gives_the_c_frequencies() {
        let clocks = C_BUILD.clocks();
        assert_eq!(clocks.system, Hertz::from_mhz(120));
        assert_eq!(clocks.ahb, Hertz::from_mhz(120));
        assert_eq!(clocks.apb1, Hertz::from_mhz(60));
        assert_eq!(clocks.apb2, Hertz::from_mhz(120));
        assert_eq!(clocks.apb1_timers, Hertz::from_mhz(120));
        assert_eq!(clocks.apb2_timers, Hertz::from_mhz(120));
    }

    #[test]
    fn the_c_configuration_gives_the_c_register_values() {
        assert_eq!(C_BUILD.pll_multiplier.field(), 0x0834_0000);
        assert_eq!(C_BUILD.ahb_divider.field() << CFG0_AHBPSC_SHIFT, 0);
        assert_eq!(C_BUILD.apb2_divider.field() << CFG0_APB2PSC_SHIFT, 0);
        assert_eq!(C_BUILD.apb1_divider.field() << CFG0_APB1PSC_SHIFT, 0x400);
        assert_eq!(AhbDivider::By2.field() << CFG0_AHBPSC_SHIFT, 0x80);
        assert_eq!(
            CFG0_PLLMF_LOW | CFG0_PLLMF_4 | CFG0_PLLMF_5,
            0x4800_0000 | 0x003C_0000
        );
    }

    #[test]
    fn pll_multipliers_follow_the_library_encoding() {
        let field = |multiplier| PllMultiplier(multiplier).field();
        assert_eq!(field(2), 0);
        assert_eq!(field(16), 14 << 18);
        assert_eq!(field(17), 1 << 27);
        assert_eq!(field(32), 1 << 27 | 15 << 18);
        assert_eq!(field(33), 1 << 30);
        assert_eq!(field(48), 1 << 30 | 15 << 18);
        assert_eq!(field(49), 1 << 27 | 1 << 30);
        assert_eq!(field(63), 1 << 27 | 1 << 30 | 14 << 18);
    }
}
