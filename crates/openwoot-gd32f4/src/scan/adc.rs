#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "register addresses fit 32 bits on this family, and the per-sample steps are inlined \
              into the scan interrupt to keep it short"
)]

use crate::{
    interrupt::{self, Priority},
    pac, rcu,
    register::{clear_bits, modify_bits, read_bits, set_bits},
    time::microseconds,
};

pub(super) const MAX_RANKS: usize = 16;
pub(super) const MAX_INPUTS: usize = (CTL0_DISNUM >> CTL0_DISNUM_SHIFT) as usize + 1;
pub(super) const MAX_CHANNEL: u8 = 18;

const _: () = assert!(MAX_INPUTS <= MAX_RANKS);

const SETTLE_US: u16 = 1000;
const WAIT_SPINS: u32 = 1 << 22;
const APB2EN_ADC0EN: u32 = 1 << 8;
const STAT_EOC: u32 = 1 << 1;
const STAT_ROVF: u32 = 1 << 5;
const CTL0_EOCIE: u32 = 1 << 5;
const CTL0_DISRC: u32 = 1 << 11;
const CTL0_DISIC: u32 = 1 << 12;
const CTL0_DISNUM_SHIFT: u32 = 13;
const CTL0_DISNUM: u32 = 0b111 << CTL0_DISNUM_SHIFT;
const CTL0_DRES: u32 = 0b11 << 24;
const CTL1_ADCON: u32 = 1 << 0;
const CTL1_CLB: u32 = 1 << 2;
const CTL1_RSTCLB: u32 = 1 << 3;
const CTL1_DMA: u32 = 1 << 8;
const CTL1_DDM: u32 = 1 << 9;
const CTL1_DAL: u32 = 1 << 11;
const CTL1_ETSRC: u32 = 0xF << 24;
const CTL1_ETSRC_TIMER1_CH1: u32 = 3 << 24;
const CTL1_ETSRC_TIMER0_CH0: u32 = 0;
const CTL1_ETMRC: u32 = 0b11 << 28;
const CTL1_ETMRC_RISING: u32 = 1 << 28;
const RSQ0_RL_SHIFT: u32 = 20;
const RSQ0_RL: u32 = 0xF << RSQ0_RL_SHIFT;
const SYNCCTL_SYNCM: u32 = 0x1F;
const SYNCCTL_ADCCK: u32 = 0b111 << 16;
const SYNCCTL_ADCCK_HCLK_DIV5: u32 = 4 << 16;
const RANK_BITS: u32 = 5;
const RANK_MASK: u32 = 0x1F;
const SAMPLE_TIME_BITS: u32 = 3;
const SAMPLE_TIME_MASK: u32 = 0b111;
const SAMPLE_TIME_SPLIT: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleTime {
    Cycles3,
    Cycles15,
    Cycles28,
    Cycles56,
    Cycles84,
    Cycles112,
    Cycles144,
    Cycles480,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Trigger {
    Timer,
    Idle,
}

impl Trigger {
    #[inline(always)]
    const fn source(self) -> u32 {
        match self {
            Self::Timer => CTL1_ETSRC_TIMER1_CH1,
            Self::Idle => CTL1_ETSRC_TIMER0_CH0,
        }
    }
}

pub(super) fn configure() {
    rcu::with_registers(|rcu| set_bits(rcu.apb2en(), APB2EN_ADC0EN));
    let common = adc_common();
    modify_bits(common.syncctl(), |bits| {
        (bits & !SYNCCTL_ADCCK) | SYNCCTL_ADCCK_HCLK_DIV5
    });
    let adc = adc();
    clear_bits(adc.ctl0(), CTL0_DRES);
    clear_bits(common.syncctl(), SYNCCTL_SYNCM);
    clear_bits(adc.ctl1(), CTL1_DAL);
    clear_bits(adc.rsq0(), RSQ0_RL);
    select_trigger(Trigger::Timer);
    modify_bits(adc.ctl1(), |bits| (bits & !CTL1_ETMRC) | CTL1_ETMRC_RISING);
    interrupt::enable(pac::Interrupt::ADC, Priority::SCAN);
    set_bits(adc.ctl0(), CTL0_EOCIE);
    if read_bits(adc.ctl1()) & CTL1_ADCON == 0 {
        set_bits(adc.ctl1(), CTL1_ADCON);
    }
    wait_microseconds(SETTLE_US);
    set_bits(adc.ctl1(), CTL1_RSTCLB);
    wait_while(|| read_bits(adc.ctl1()) & CTL1_RSTCLB != 0);
    set_bits(adc.ctl1(), CTL1_CLB);
    wait_while(|| read_bits(adc.ctl1()) & CTL1_CLB != 0);
    set_bits(adc.ctl1(), CTL1_DDM);
    set_bits(adc.ctl1(), CTL1_DMA);
}

#[inline(always)]
pub(super) fn convert_at_rank(rank: u8, channel: u8, sample_time: u32) {
    let adc = adc();
    let rank = u32::from(rank);
    let channel_field = |slot: u32| {
        let shift = RANK_BITS * slot;
        move |bits: u32| (bits & !(RANK_MASK << shift)) | u32::from(channel) << shift
    };
    match rank {
        0..6 => modify_bits(adc.rsq2(), channel_field(rank)),
        6..12 => modify_bits(adc.rsq1(), channel_field(rank - 6)),
        _ => modify_bits(adc.rsq0(), channel_field(rank - 12)),
    }
    let time_field = |slot: u8| {
        let shift = SAMPLE_TIME_BITS * u32::from(slot);
        move |bits: u32| (bits & !(SAMPLE_TIME_MASK << shift)) | sample_time << shift
    };
    if channel < SAMPLE_TIME_SPLIT {
        modify_bits(adc.sampt1(), time_field(channel));
    } else {
        modify_bits(adc.sampt0(), time_field(channel - SAMPLE_TIME_SPLIT));
    }
}

pub(super) fn configure_sequence(length: u32) {
    let adc = adc();
    let last = length.saturating_sub(1);
    modify_bits(adc.rsq0(), |bits| {
        (bits & !RSQ0_RL) | (last << RSQ0_RL_SHIFT) & RSQ0_RL
    });
    clear_bits(adc.ctl0(), CTL0_DISRC | CTL0_DISIC);
    clear_bits(adc.ctl0(), CTL0_DISNUM);
    set_bits(adc.ctl0(), (last << CTL0_DISNUM_SHIFT) & CTL0_DISNUM);
    set_bits(adc.ctl0(), CTL0_DISRC);
}

#[inline(always)]
pub(super) fn acknowledge() {
    clear_bits(adc().stat(), STAT_EOC);
}

#[inline(always)]
pub(super) fn take_overflow() -> bool {
    let adc = adc();
    if read_bits(adc.stat()) & STAT_ROVF == 0 {
        return false;
    }
    clear_bits(adc.stat(), STAT_ROVF);
    true
}

pub(super) fn restart_transport(restart: impl FnOnce()) {
    let adc = adc();
    clear_bits(adc.ctl1(), CTL1_DMA);
    restart();
    set_bits(adc.ctl1(), CTL1_DMA);
}

#[inline(always)]
pub(super) fn clear_trigger() {
    clear_bits(adc().ctl1(), CTL1_ETSRC);
}

#[inline(always)]
pub(super) fn set_trigger(trigger: Trigger) {
    set_bits(adc().ctl1(), trigger.source());
}

pub(super) fn select_trigger(trigger: Trigger) {
    modify_bits(adc().ctl1(), |bits| (bits & !CTL1_ETSRC) | trigger.source());
}

pub(super) fn sample_address() -> u32 {
    adc().rdata().as_ptr().addr() as u32
}

pub(super) const fn sample_time_bits(sample_time: SampleTime) -> u32 {
    match sample_time {
        SampleTime::Cycles3 => 0,
        SampleTime::Cycles15 => 1,
        SampleTime::Cycles28 => 2,
        SampleTime::Cycles56 => 3,
        SampleTime::Cycles84 => 4,
        SampleTime::Cycles112 => 5,
        SampleTime::Cycles144 => 6,
        SampleTime::Cycles480 => 7,
    }
}

fn wait_microseconds(duration: u16) {
    let start = microseconds();
    wait_while(|| microseconds().wrapping_sub(start) < duration);
}

fn wait_while(mut busy: impl FnMut() -> bool) {
    let mut spins = 0;
    while busy() && spins < WAIT_SPINS {
        spins += 1;
    }
}

#[inline(always)]
fn adc() -> &'static pac::adc0::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Adc0`; from then on only this module touches its
    // registers: from the scan interrupt and, between cycles, from the context that holds the
    // scanner. The interrupt's `acknowledge` can run between two register accesses of that
    // context; every access is one volatile read or write through a shared reference, so
    // neither invalidates what the other holds.
    unsafe { &*pac::Adc0::ptr() }
}

fn adc_common() -> &'static pac::adc_common::RegisterBlock {
    // SAFETY: `Converter::new` consumed `AdcCommon`; only `configure` writes these registers.
    unsafe { &*pac::AdcCommon::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_times_carry_the_reference_manual_encodings() {
        assert_eq!(sample_time_bits(SampleTime::Cycles3), 0b000);
        assert_eq!(sample_time_bits(SampleTime::Cycles56), 0b011);
        assert_eq!(sample_time_bits(SampleTime::Cycles480), 0b111);
    }

    #[test]
    fn a_trigger_converts_no_more_inputs_than_its_count_field_holds() {
        assert_eq!(MAX_INPUTS, 8);
        let last = (MAX_INPUTS as u32 - 1) << CTL0_DISNUM_SHIFT;
        assert_eq!(last & CTL0_DISNUM, last);
        let beyond = (MAX_INPUTS as u32) << CTL0_DISNUM_SHIFT;
        assert_ne!(beyond & CTL0_DISNUM, beyond);
    }

    #[test]
    fn a_wait_ends_when_its_condition_does_or_after_its_bound() {
        let mut polls = 0_u32;
        wait_while(|| {
            polls += 1;
            polls < 5
        });
        assert_eq!(polls, 5);
        let mut polls = 0_u32;
        wait_while(|| {
            polls += 1;
            true
        });
        assert_eq!(polls, WAIT_SPINS + 1);
    }
}
