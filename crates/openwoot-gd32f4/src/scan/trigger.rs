#![allow(
    clippy::inline_always,
    reason = "the trigger steps are inlined into the scan interrupt to keep it short"
)]

use crate::{
    pac, rcu,
    rcu::Clocks,
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
    time,
};

const APB1EN_TIMER1EN: u32 = 1 << 0;
const APB1RST_TIMER1RST: u32 = 1 << 0;
const PERIOD: u32 = 0xFFFF;
const CTL0_CEN: u32 = 1 << 0;
const CTL0_SPM: u32 = 1 << 3;
const CTL0_DIR_CAM: u32 = 0b111 << 4;
const CTL0_ARSE: u32 = 1 << 7;
const CTL0_CKDIV: u32 = 0b11 << 8;
const SWEVG_UPG: u32 = 1 << 0;
const CHCTL0_CH1MS: u32 = 0b11 << 8;
const CHCTL0_CH1COMSEN: u32 = 1 << 11;
const CHCTL0_CH1COMCTL: u32 = 0b111 << 12;
const CHCTL0_CH1COMCTL_PWM1: u32 = 0b111 << 12;
const CHCTL2_CH1EN: u32 = 1 << 4;
const CHCTL2_CH1P: u32 = 1 << 5;
const INTF_UPIF: u32 = 1 << 0;
const INTF_CH1IF: u32 = 1 << 2;

pub(super) fn configure(clocks: &Clocks) {
    rcu::with_registers(|rcu| {
        set_bits(rcu.apb1en(), APB1EN_TIMER1EN);
        set_bits(rcu.apb1rst(), APB1RST_TIMER1RST);
        clear_bits(rcu.apb1rst(), APB1RST_TIMER1RST);
    });
    let timer = timer();
    write_bits(timer.psc(), time::prescaler(clocks.apb1_timers));
    clear_bits(timer.ctl0(), CTL0_DIR_CAM);
    write_bits(timer.car(), PERIOD);
    clear_bits(timer.ctl0(), CTL0_CKDIV);
    write_bits(timer.swevg(), SWEVG_UPG);
    clear_bits(timer.ctl0(), CTL0_ARSE);
    clear_bits(timer.chctl2(), CHCTL2_CH1EN);
    clear_bits(timer.chctl0_output(), CHCTL0_CH1MS);
    set_bits(timer.chctl2(), CHCTL2_CH1EN);
    clear_bits(timer.chctl2(), CHCTL2_CH1P);
    modify_bits(timer.chctl0_output(), |bits| {
        (bits & !CHCTL0_CH1COMCTL) | CHCTL0_CH1COMCTL_PWM1
    });
    clear_bits(timer.chctl0_output(), CHCTL0_CH1COMSEN);
    set_bits(timer.ctl0(), CTL0_SPM);
}

#[inline(always)]
pub(super) fn stop() {
    clear_bits(timer().ctl0(), CTL0_CEN);
}

#[inline(always)]
pub(super) fn start() {
    set_bits(timer().ctl0(), CTL0_CEN);
}

#[inline(always)]
pub(super) fn set_counter(count: u32) {
    write_bits(timer().cnt(), count);
}

#[inline(always)]
pub(super) fn set_compare(compare: u32) {
    write_bits(timer().ch1cv(), compare);
}

pub(super) fn counter() -> u32 {
    read_bits(timer().cnt())
}

pub(super) fn fired() -> bool {
    read_bits(timer().intf()) & (INTF_UPIF | INTF_CH1IF) != 0
}

pub(super) fn clear_flags() {
    let timer = timer();
    write_bits(timer.intf(), !INTF_CH1IF);
    write_bits(timer.intf(), !INTF_UPIF);
}

#[inline(always)]
fn timer() -> &'static pac::timer1::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Timer1`; only this module touches its registers, from the
    // scan interrupt or between cycles, which never overlap.
    unsafe { &*pac::Timer1::ptr() }
}
