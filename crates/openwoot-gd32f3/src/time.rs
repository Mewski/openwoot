#[cfg(target_os = "none")]
mod driver;
#[cfg_attr(
    not(any(test, target_os = "none")),
    allow(
        dead_code,
        reason = "the time driver that uses it exists only on the target"
    )
)]
mod monotonic;

use crate::{
    interrupt::{self, Priority},
    pac,
    rcu::{self, Clocks, Hertz},
    register::{read_bits, set_bits, write_bits},
};

pub const TICK_HZ: u32 = 1_000_000;

const COUNTER_BITS: u32 = 16;
const APB1EN_TIMER4EN: u32 = 1 << 3;
const CTL0_CEN: u32 = 1 << 0;
const SWEVG_UPG: u32 = 1 << 0;
const DMAINTEN_UPIE: u32 = 1 << 0;
const DMAINTEN_CH0IE: u32 = 1 << 1;

#[expect(
    clippy::needless_pass_by_value,
    reason = "taking the peripheral hands the timer to the time driver for good"
)]
pub fn start(timer: pac::Timer4, clocks: &Clocks) {
    rcu::with_registers(|rcu| set_bits(rcu.apb1en(), APB1EN_TIMER4EN));
    write_bits(timer.psc(), prescaler(clocks.apb1_timers));
    write_bits(timer.car(), monotonic::COUNTER_MAX);
    write_bits(timer.swevg(), SWEVG_UPG);
    write_bits(timer.intf(), 0);
    write_bits(timer.ch0cv(), monotonic::HALF);
    write_bits(timer.dmainten(), DMAINTEN_UPIE | DMAINTEN_CH0IE);
    interrupt::enable(pac::Interrupt::TIMER4, Priority::TIME);
    write_bits(timer.ctl0(), CTL0_CEN);
}

#[must_use]
pub const fn prescaler(timer_clock: Hertz) -> u32 {
    timer_clock.0 / TICK_HZ - 1
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "the scan reads the microsecond count as its low 16 bits as in the C HAL, inlined into \
              the scan interrupt as the C HAL's always_inline helper"
)]
#[inline(always)]
pub(crate) fn microseconds() -> u16 {
    read_bits(registers().cnt()) as u16
}

#[allow(
    clippy::inline_always,
    reason = "the scan's microsecond read runs inside the scan interrupt"
)]
#[inline(always)]
fn registers() -> &'static pac::timer1::RegisterBlock {
    // SAFETY: `start` consumed the `Timer4` peripheral; from then on only the time driver writes
    // these registers, and every read-modify-write runs inside a critical section or in the timer
    // interrupt, which the critical section masks. `microseconds` only reads the counter, which
    // changes nothing.
    unsafe { &*pac::Timer4::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prescaler_divides_the_timer_clock_down_to_one_megahertz() {
        assert_eq!(prescaler(Hertz::from_mhz(120)), 119);
        assert_eq!(prescaler(Hertz::from_mhz(60)), 59);
    }
}
