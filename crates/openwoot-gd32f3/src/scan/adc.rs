#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "ranks, sequence lengths and the sample register address fit their register fields \
              on this family, and the per-sample steps are inlined into the scan interrupt as the \
              C HAL's always_inline helpers"
)]

use crate::{
    interrupt::{self, Priority},
    pac, rcu,
    register::{clear_bits, modify_bits, read_bits, set_bits},
    time::microseconds,
};

pub(super) const MAX_RANKS: usize = 16;

const SETTLE_US: u16 = 1000;

const APB2EN_ADC0EN: u32 = 1 << 9;
const APB2EN_ADC1EN: u32 = 1 << 10;
const CFG0_ADCPSC: u32 = 0b11 << 14;
const CFG0_ADCPSC_APB2_DIV4: u32 = 0b01 << 14;
const CFG0_ADCPSC_2: u32 = 1 << 28;
const CFG1_ADCPSC_3: u32 = 1 << 29;

const ADC_STAT_EOC: u32 = 1 << 1;
const ADC_CTL0_EOCIE: u32 = 1 << 5;
const ADC_CTL0_DISRC: u32 = 1 << 11;
const ADC_CTL0_DISIC: u32 = 1 << 12;
const ADC_CTL0_DISNUM_SHIFT: u32 = 13;
const ADC_CTL0_DISNUM: u32 = 0b111 << ADC_CTL0_DISNUM_SHIFT;
const ADC_CTL0_SYNCM: u32 = 0xF << 16;
const ADC_CTL0_SYNCM_REGULAR_PARALLEL: u32 = 6 << 16;
const ADC_CTL1_ADCON: u32 = 1 << 0;
const ADC_CTL1_CLB: u32 = 1 << 2;
const ADC_CTL1_RSTCLB: u32 = 1 << 3;
const ADC_CTL1_DMA: u32 = 1 << 8;
const ADC_CTL1_DAL: u32 = 1 << 11;
const ADC_CTL1_ETSRC: u32 = 0b111 << 17;
const ADC_CTL1_ETSRC_TIMER1_CH1: u32 = 3 << 17;
const ADC_CTL1_ETSRC_SOFTWARE: u32 = 7 << 17;
const ADC_CTL1_ETERC: u32 = 1 << 20;
const ADC_RSQ0_RL_SHIFT: u32 = 20;
const ADC_RSQ0_RL: u32 = 0xF << ADC_RSQ0_RL_SHIFT;
const ADC_OVSAMPCTL_OVSEN: u32 = 1 << 0;
const ADC_OVSAMPCTL_OVSR_SHIFT: u32 = 2;
const ADC_OVSAMPCTL_OVSR: u32 = 0b111 << ADC_OVSAMPCTL_OVSR_SHIFT;
const ADC_OVSAMPCTL_OVSS_SHIFT: u32 = 5;
const ADC_OVSAMPCTL_OVSS: u32 = 0xF << ADC_OVSAMPCTL_OVSS_SHIFT;
const ADC_OVSAMPCTL_TOVS: u32 = 1 << 9;
const ADC_OVSAMPCTL_DRES: u32 = 0b11 << 12;
const ADC_RANK_BITS: u32 = 5;
const ADC_RANK_MASK: u32 = 0x1F;
const ADC_SAMPLE_TIME_BITS: u32 = 3;
const ADC_SAMPLE_TIME_MASK: u32 = 0b111;
const ADC_SAMPLE_TIME_SPLIT: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleTime {
    HalfCycles3,
    HalfCycles15,
    HalfCycles27,
    HalfCycles57,
    HalfCycles83,
    HalfCycles111,
    HalfCycles143,
    HalfCycles479,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Oversampling {
    Times2,
    Times4,
    Times8,
    Times16,
    Times32,
    Times64,
    Times128,
    Times256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Trigger {
    Timer,
    Software,
}

impl Trigger {
    #[inline(always)]
    const fn bits(self) -> u32 {
        match self {
            Self::Timer => ADC_CTL1_ETSRC_TIMER1_CH1,
            Self::Software => ADC_CTL1_ETSRC_SOFTWARE,
        }
    }
}

macro_rules! configure_sequence {
    ($adc:expr, $length:expr) => {{
        let adc = $adc;
        let last: u32 = ($length as u32).saturating_sub(1);
        modify_bits(adc.rsq0(), |bits| {
            (bits & !ADC_RSQ0_RL) | (last << ADC_RSQ0_RL_SHIFT) & ADC_RSQ0_RL
        });
        clear_bits(adc.ctl0(), ADC_CTL0_DISRC | ADC_CTL0_DISIC);
        clear_bits(adc.ctl0(), ADC_CTL0_DISNUM);
        set_bits(
            adc.ctl0(),
            (last << ADC_CTL0_DISNUM_SHIFT) & ADC_CTL0_DISNUM,
        );
        set_bits(adc.ctl0(), ADC_CTL0_DISRC);
    }};
}

macro_rules! convert_at_rank {
    ($adc:expr, $rank:expr, $channel:expr, $sample_time:expr) => {{
        let adc = $adc;
        let rank = u32::from($rank);
        let channel: u8 = $channel;
        let channel_field = |slot: u32| {
            let shift = ADC_RANK_BITS * slot;
            move |bits: u32| (bits & !(ADC_RANK_MASK << shift)) | u32::from(channel) << shift
        };
        match rank {
            0..6 => modify_bits(adc.rsq2(), channel_field(rank)),
            6..12 => modify_bits(adc.rsq1(), channel_field(rank - 6)),
            _ => modify_bits(adc.rsq0(), channel_field(rank - 12)),
        }
        let sample_time: u32 = $sample_time;
        let time_field = |slot: u8| {
            let shift = ADC_SAMPLE_TIME_BITS * u32::from(slot);
            move |bits: u32| (bits & !(ADC_SAMPLE_TIME_MASK << shift)) | sample_time << shift
        };
        if channel < ADC_SAMPLE_TIME_SPLIT {
            modify_bits(adc.sampt1(), time_field(channel));
        } else {
            modify_bits(adc.sampt0(), time_field(channel - ADC_SAMPLE_TIME_SPLIT));
        }
    }};
}

macro_rules! configure_oversampling {
    ($adc:expr, $shift:expr) => {{
        let adc = $adc;
        let shift: u32 = $shift;
        clear_bits(adc.ovsampctl(), ADC_OVSAMPCTL_TOVS);
        clear_bits(adc.ovsampctl(), ADC_OVSAMPCTL_OVSR | ADC_OVSAMPCTL_OVSS);
        set_bits(
            adc.ovsampctl(),
            (shift << ADC_OVSAMPCTL_OVSS_SHIFT) & ADC_OVSAMPCTL_OVSS
                | ((shift - 1) << ADC_OVSAMPCTL_OVSR_SHIFT) & ADC_OVSAMPCTL_OVSR,
        );
        set_bits(adc.ovsampctl(), ADC_OVSAMPCTL_OVSEN);
    }};
}

macro_rules! enable_and_calibrate {
    ($adc:expr) => {{
        let adc = $adc;
        if read_bits(adc.ctl1()) & ADC_CTL1_ADCON == 0 {
            set_bits(adc.ctl1(), ADC_CTL1_ADCON);
        }
        wait_microseconds(SETTLE_US);
        set_bits(adc.ctl1(), ADC_CTL1_RSTCLB);
        while read_bits(adc.ctl1()) & ADC_CTL1_RSTCLB != 0 {}
        set_bits(adc.ctl1(), ADC_CTL1_CLB);
        while read_bits(adc.ctl1()) & ADC_CTL1_CLB != 0 {}
    }};
}

pub(super) fn configure(
    inputs: usize,
    reference_channel: u8,
    sample_time: u32,
    oversampling: Oversampling,
) {
    rcu::with_registers(|rcu| {
        set_bits(rcu.apb2en(), APB2EN_ADC0EN);
        set_bits(rcu.apb2en(), APB2EN_ADC1EN);
        modify_bits(rcu.cfg0(), |bits| {
            (bits & !(CFG0_ADCPSC_2 | CFG0_ADCPSC)) | CFG0_ADCPSC_APB2_DIV4
        });
        clear_bits(rcu.cfg1(), CFG1_ADCPSC_3);
    });
    let adc = adc();
    let reference = reference_adc();
    clear_bits(adc.ovsampctl(), ADC_OVSAMPCTL_DRES);
    clear_bits(reference.ovsampctl(), ADC_OVSAMPCTL_DRES);
    modify_bits(adc.ctl0(), |bits| {
        (bits & !ADC_CTL0_SYNCM) | ADC_CTL0_SYNCM_REGULAR_PARALLEL
    });
    clear_bits(adc.ctl1(), ADC_CTL1_DAL);
    clear_bits(reference.ctl1(), ADC_CTL1_DAL);
    clear_bits(adc.rsq0(), ADC_RSQ0_RL);
    clear_bits(reference.rsq0(), ADC_RSQ0_RL);
    for rank in 0..inputs as u8 {
        convert_at_rank!(reference, rank, reference_channel, sample_time);
    }
    modify_bits(adc.ctl1(), |bits| {
        (bits & !ADC_CTL1_ETSRC) | Trigger::Timer.bits()
    });
    modify_bits(reference.ctl1(), |bits| {
        (bits & !ADC_CTL1_ETSRC) | Trigger::Software.bits()
    });
    set_bits(adc.ctl1(), ADC_CTL1_ETERC);
    set_bits(reference.ctl1(), ADC_CTL1_ETERC);
    let shift = oversampling_shift(oversampling);
    configure_oversampling!(adc, shift);
    configure_oversampling!(reference, shift);
    interrupt::enable(pac::Interrupt::ADC0_1, Priority::SCAN);
    set_bits(adc.ctl0(), ADC_CTL0_EOCIE);
    enable_and_calibrate!(adc);
    enable_and_calibrate!(reference);
    set_bits(adc.ctl1(), ADC_CTL1_DMA);
}

#[inline(always)]
pub(super) fn convert_at_rank(rank: u8, channel: u8, sample_time: u32) {
    convert_at_rank!(adc(), rank, channel, sample_time);
}

pub(super) fn configure_sequence(length: usize) {
    configure_sequence!(adc(), length);
}

pub(super) fn configure_reference_sequence(length: usize, channel: u8, sample_time: u32) {
    let reference = reference_adc();
    configure_sequence!(reference, length);
    for rank in 0..length as u8 {
        convert_at_rank!(reference, rank, channel, sample_time);
    }
}

#[inline(always)]
pub(super) fn acknowledge() {
    clear_bits(adc().stat(), ADC_STAT_EOC);
}

#[inline(always)]
pub(super) fn clear_trigger() {
    clear_bits(adc().ctl1(), ADC_CTL1_ETSRC);
}

#[inline(always)]
pub(super) fn set_trigger(trigger: Trigger) {
    set_bits(adc().ctl1(), trigger.bits());
}

pub(super) fn select_trigger(trigger: Trigger) {
    modify_bits(adc().ctl1(), |bits| {
        (bits & !ADC_CTL1_ETSRC) | trigger.bits()
    });
}

pub(super) fn sample_address() -> u32 {
    adc().rdata().as_ptr().addr() as u32
}

pub(super) const fn sample_time_bits(sample_time: SampleTime) -> u32 {
    match sample_time {
        SampleTime::HalfCycles3 => 0,
        SampleTime::HalfCycles15 => 1,
        SampleTime::HalfCycles27 => 2,
        SampleTime::HalfCycles57 => 3,
        SampleTime::HalfCycles83 => 4,
        SampleTime::HalfCycles111 => 5,
        SampleTime::HalfCycles143 => 6,
        SampleTime::HalfCycles479 => 7,
    }
}

const fn oversampling_shift(oversampling: Oversampling) -> u32 {
    match oversampling {
        Oversampling::Times2 => 1,
        Oversampling::Times4 => 2,
        Oversampling::Times8 => 3,
        Oversampling::Times16 => 4,
        Oversampling::Times32 => 5,
        Oversampling::Times64 => 6,
        Oversampling::Times128 => 7,
        Oversampling::Times256 => 8,
    }
}

fn wait_microseconds(duration: u16) {
    let start = microseconds();
    while microseconds().wrapping_sub(start) < duration {}
}

#[inline(always)]
fn adc() -> &'static pac::adc0::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Adc0`; from then on only this module touches its
    // registers, from the scan interrupt or between cycles, which never overlap.
    unsafe { &*pac::Adc0::ptr() }
}

fn reference_adc() -> &'static pac::adc1::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Adc1`; only this module configures it, between cycles.
    unsafe { &*pac::Adc1::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_times_oversampling_shifts_three_bits() {
        assert_eq!(oversampling_shift(Oversampling::Times8), 3);
    }
}
