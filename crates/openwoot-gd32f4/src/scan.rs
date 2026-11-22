#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "addresses and register fields fit 32 bits on this family, the microsecond count is \
              read as its low 16 bits as in the C HAL, and the per-sample steps are inlined into \
              the scan interrupt as the C HAL's always_inline helpers"
)]

use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU32, Ordering},
};

use cortex_m::peripheral::NVIC;
use openwoot::scan::{self, Sample};

use crate::{
    interrupt::{self, Binding, Handler, Priority, typelevel},
    pac, rcu,
    rcu::Clocks,
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

const MAX_RANKS: usize = 16;
const ADDRESS_LINES: usize = 4;
const ADDRESSES: usize = 1 << ADDRESS_LINES;
const PORT_STRIDE: usize = 0x400;
const RESET_SHIFT: u32 = 16;
const SETTLE_US: u16 = 1000;

const AHB1EN_DMA1EN: u32 = 1 << 22;
const APB1EN_TIMER1EN: u32 = 1 << 0;
const APB1RST_TIMER1RST: u32 = 1 << 0;
const APB2EN_ADC0EN: u32 = 1 << 8;

const GPIO_MODE_OUTPUT: u32 = 0b01;
const GPIO_MODE_ANALOG: u32 = 0b11;
const GPIO_SPEED_50MHZ: u32 = 0b10;

const TIMER_PERIOD: u32 = 0xFFFF;
const TIMER_CTL0_CEN: u32 = 1 << 0;
const TIMER_CTL0_SPM: u32 = 1 << 3;
const TIMER_CTL0_DIR_CAM: u32 = 0b111 << 4;
const TIMER_CTL0_ARSE: u32 = 1 << 7;
const TIMER_CTL0_CKDIV: u32 = 0b11 << 8;
const TIMER_SWEVG_UPG: u32 = 1 << 0;
const TIMER_CHCTL0_CH1MS: u32 = 0b11 << 8;
const TIMER_CHCTL0_CH1COMSEN: u32 = 1 << 11;
const TIMER_CHCTL0_CH1COMCTL: u32 = 0b111 << 12;
const TIMER_CHCTL0_CH1COMCTL_PWM1: u32 = 0b111 << 12;
const TIMER_CHCTL2_CH1EN: u32 = 1 << 4;
const TIMER_CHCTL2_CH1P: u32 = 1 << 5;
const TIMER_INTF_UPIF: u32 = 1 << 0;
const TIMER_INTF_CH1IF: u32 = 1 << 2;

const ADC_STAT_EOC: u32 = 1 << 1;
const ADC_CTL0_EOCIE: u32 = 1 << 5;
const ADC_CTL0_DISRC: u32 = 1 << 11;
const ADC_CTL0_DISIC: u32 = 1 << 12;
const ADC_CTL0_DISNUM_SHIFT: u32 = 13;
const ADC_CTL0_DISNUM: u32 = 0b111 << ADC_CTL0_DISNUM_SHIFT;
const ADC_CTL0_DRES: u32 = 0b11 << 24;
const ADC_CTL1_ADCON: u32 = 1 << 0;
const ADC_CTL1_CLB: u32 = 1 << 2;
const ADC_CTL1_RSTCLB: u32 = 1 << 3;
const ADC_CTL1_DMA: u32 = 1 << 8;
const ADC_CTL1_DDM: u32 = 1 << 9;
const ADC_CTL1_DAL: u32 = 1 << 11;
const ADC_CTL1_ETSRC: u32 = 0xF << 24;
const ADC_CTL1_ETSRC_TIMER1_CH1: u32 = 3 << 24;
const ADC_CTL1_ETSRC_TIMER0_CH0: u32 = 0;
const ADC_CTL1_ETMRC: u32 = 0b11 << 28;
const ADC_CTL1_ETMRC_RISING: u32 = 1 << 28;
const ADC_RSQ0_RL_SHIFT: u32 = 20;
const ADC_RSQ0_RL: u32 = 0xF << ADC_RSQ0_RL_SHIFT;
const ADC_SYNCCTL_SYNCM: u32 = 0x1F;
const ADC_SYNCCTL_ADCCK: u32 = 0b111 << 16;
const ADC_SYNCCTL_ADCCK_HCLK_DIV5: u32 = 4 << 16;
const ADC_RANK_BITS: u32 = 5;
const ADC_RANK_MASK: u32 = 0x1F;
const ADC_SAMPLE_TIME_BITS: u32 = 3;
const ADC_SAMPLE_TIME_MASK: u32 = 0b111;
const ADC_SAMPLE_TIME_SPLIT: u8 = 10;

const DMA_CHCTL_CHEN: u32 = 1 << 0;
const DMA_CHCTL_TM: u32 = 0b11 << 6;
const DMA_CHCTL_CMEN: u32 = 1 << 8;
const DMA_CHCTL_PNAGA: u32 = 1 << 9;
const DMA_CHCTL_MNAGA: u32 = 1 << 10;
const DMA_CHCTL_PWIDTH: u32 = 0b11 << 11;
const DMA_CHCTL_PWIDTH_32: u32 = 0b10 << 11;
const DMA_CHCTL_MWIDTH: u32 = 0b11 << 13;
const DMA_CHCTL_MWIDTH_32: u32 = 0b10 << 13;
const DMA_CHCTL_PRIO: u32 = 0b11 << 16;
const DMA_CHCTL_PRIO_HIGH: u32 = 0b10 << 16;
const DMA_CHCTL_PERIEN: u32 = 0b111 << 25;
const DMA_CHFCTL_MDMEN: u32 = 1 << 2;
const DMA_CHANNEL0_FLAGS: u32 = 0x3D;

static SAMPLES: [AtomicU32; MAX_RANKS] = [const { AtomicU32::new(0) }; MAX_RANKS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    port: u8,
    pin: u8,
}

impl Line {
    #[must_use]
    pub const fn new(port: char, pin: u8) -> Self {
        assert!(
            port >= 'A' && port <= 'I' && pin < 16,
            "a pin of ports A to I"
        );
        Self {
            port: port as u8 - b'A',
            pin,
        }
    }

    const fn mask(self) -> u32 {
        1 << self.pin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalogInput {
    pub line: Line,
    pub channel: u8,
}

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
pub struct Wiring<const INPUTS: usize> {
    pub address_port: char,
    pub address_pins: [u8; ADDRESS_LINES],
    pub enable: [Line; INPUTS],
    pub inputs: [AnalogInput; INPUTS],
    pub sample_time: SampleTime,
}

#[derive(Debug)]
pub struct Peripherals {
    pub adc: pac::Adc0,
    pub adc_common: pac::AdcCommon,
    pub timer: pac::Timer1,
}

pub trait Conversion {
    fn on_conversion();
}

#[derive(Debug)]
pub struct InterruptHandler<Hook>(PhantomData<Hook>);

impl<Hook: Conversion> Handler<typelevel::ADC> for InterruptHandler<Hook> {
    // SAFETY: the handler runs only in the ADC interrupt, as `Handler` requires; the hook it calls
    // is safe code.
    #[inline(always)]
    unsafe fn on_interrupt() {
        Hook::on_conversion();
    }
}

#[derive(Debug)]
pub struct Converter<const INPUTS: usize> {
    address_port: u8,
    address_values: [u32; ADDRESSES],
    enable: [Line; INPUTS],
    channels: [u8; INPUTS],
    sample_time: u32,
}

impl<const INPUTS: usize> Converter<INPUTS> {
    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "taking the peripherals hands them to the scan for good"
    )]
    pub fn new<Hook: Conversion>(
        peripherals: Peripherals,
        wiring: &Wiring<INPUTS>,
        clocks: &Clocks,
        _irqs: impl Binding<typelevel::ADC, InterruptHandler<Hook>>,
    ) -> Self {
        const { assert!(INPUTS > 0 && INPUTS <= MAX_RANKS) };
        let Peripherals {
            adc: _,
            adc_common: _,
            timer: _,
        } = peripherals;
        let address_port = wiring.address_port as u8 - b'A';
        let converter = Self {
            address_port,
            address_values: address_values(wiring.address_pins),
            enable: wiring.enable,
            channels: wiring.inputs.map(|input| input.channel),
            sample_time: sample_time_bits(wiring.sample_time),
        };
        converter.configure_outputs(wiring);
        configure_trigger(clocks);
        Self::configure_converter(wiring);
        converter
    }

    fn configure_outputs(&self, wiring: &Wiring<INPUTS>) {
        for pin in wiring.address_pins {
            configure_output(
                Line {
                    port: self.address_port,
                    pin,
                },
                false,
            );
        }
        for line in wiring.enable {
            configure_output(line, true);
        }
    }

    fn configure_converter(wiring: &Wiring<INPUTS>) {
        start_transport(1);
        for input in wiring.inputs {
            configure_analog(input.line);
        }
        rcu::with_registers(|rcu| set_bits(rcu.apb2en(), APB2EN_ADC0EN));
        let common = adc_common();
        modify_bits(common.syncctl(), |bits| {
            (bits & !ADC_SYNCCTL_ADCCK) | ADC_SYNCCTL_ADCCK_HCLK_DIV5
        });
        let adc = adc();
        clear_bits(adc.ctl0(), ADC_CTL0_DRES);
        clear_bits(common.syncctl(), ADC_SYNCCTL_SYNCM);
        clear_bits(adc.ctl1(), ADC_CTL1_DAL);
        clear_bits(adc.rsq0(), ADC_RSQ0_RL);
        modify_bits(adc.ctl1(), |bits| {
            (bits & !ADC_CTL1_ETSRC) | ADC_CTL1_ETSRC_TIMER1_CH1
        });
        modify_bits(adc.ctl1(), |bits| {
            (bits & !ADC_CTL1_ETMRC) | ADC_CTL1_ETMRC_RISING
        });
        interrupt::enable(pac::Interrupt::ADC, Priority::SCAN);
        set_bits(adc.ctl0(), ADC_CTL0_EOCIE);
        if read_bits(adc.ctl1()) & ADC_CTL1_ADCON == 0 {
            set_bits(adc.ctl1(), ADC_CTL1_ADCON);
        }
        wait_microseconds(SETTLE_US);
        set_bits(adc.ctl1(), ADC_CTL1_RSTCLB);
        while read_bits(adc.ctl1()) & ADC_CTL1_RSTCLB != 0 {}
        set_bits(adc.ctl1(), ADC_CTL1_CLB);
        while read_bits(adc.ctl1()) & ADC_CTL1_CLB != 0 {}
        set_bits(adc.ctl1(), ADC_CTL1_DDM);
        set_bits(adc.ctl1(), ADC_CTL1_DMA);
    }

    #[inline(always)]
    fn enable_line(&self, input: u8) -> Option<Line> {
        self.enable.get(usize::from(input)).copied()
    }

    #[inline(always)]
    fn convert_at_rank(&self, rank: u8, input: u8) {
        let Some(&channel) = self.channels.get(usize::from(input)) else {
            return;
        };
        let adc = adc();
        let rank = u32::from(rank);
        let channel_field = |slot: u32| {
            let shift = ADC_RANK_BITS * slot;
            move |bits: u32| (bits & !(ADC_RANK_MASK << shift)) | u32::from(channel) << shift
        };
        match rank {
            0..6 => modify_bits(adc.rsq2(), channel_field(rank)),
            6..12 => modify_bits(adc.rsq1(), channel_field(rank - 6)),
            _ => modify_bits(adc.rsq0(), channel_field(rank - 12)),
        }
        let sample_time = self.sample_time;
        let time_field = |slot: u8| {
            let shift = ADC_SAMPLE_TIME_BITS * u32::from(slot);
            move |bits: u32| (bits & !(ADC_SAMPLE_TIME_MASK << shift)) | sample_time << shift
        };
        if channel < ADC_SAMPLE_TIME_SPLIT {
            modify_bits(adc.sampt1(), time_field(channel));
        } else {
            modify_bits(adc.sampt0(), time_field(channel - ADC_SAMPLE_TIME_SPLIT));
        }
    }

    fn all_inputs_off(&self) {
        for line in self.enable {
            write_bits(gpio(line.port).bop(), line.mask());
        }
    }

    fn all_inputs_on(&self, order: &[u8]) {
        for (rank, &input) in (0_u8..).zip(order) {
            self.convert_at_rank(rank, input);
            if let Some(line) = self.enable_line(input) {
                write_bits(gpio(line.port).bc(), line.mask());
            }
        }
    }
}

impl<const INPUTS: usize> scan::Converter for Converter<INPUTS> {
    #[inline(always)]
    fn acknowledge() {
        clear_bits(adc().stat(), ADC_STAT_EOC);
    }

    #[inline(always)]
    fn request_interrupt() {
        NVIC::pend(pac::Interrupt::ADC);
    }

    #[inline(always)]
    fn sample(&self, rank: usize) -> Sample {
        let word = SAMPLES
            .get(rank)
            .map_or(0, |sample| sample.load(Ordering::Relaxed));
        Sample {
            reading: word as u16,
            reference: 0,
        }
    }

    #[inline(always)]
    fn select_address(&mut self, address: u8) {
        let value = self
            .address_values
            .get(usize::from(address))
            .copied()
            .unwrap_or_default();
        write_bits(gpio(self.address_port).bop(), value);
    }

    #[inline(always)]
    fn select_input(&mut self, previous: u8, next: u8) {
        if let Some(line) = self.enable_line(previous) {
            write_bits(gpio(line.port).bop(), line.mask());
        }
        if let Some(line) = self.enable_line(next) {
            write_bits(gpio(line.port).bc(), line.mask());
        }
        self.convert_at_rank(0, next);
    }

    fn enter_single(&mut self, previous: u8) {
        self.all_inputs_off();
        if let Some(line) = self.enable_line(previous) {
            write_bits(gpio(line.port).bop(), line.mask());
        }
        if let Some(line) = self.enable_line(0) {
            write_bits(gpio(line.port).bc(), line.mask());
        }
        self.convert_at_rank(0, 0);
        self.select_address(0);
        configure_sequence(1);
        start_transport(1);
    }

    fn enter_parallel(&mut self, order: &[u8], address: u8) {
        self.all_inputs_off();
        self.all_inputs_on(order);
        self.select_address(address);
        configure_sequence(INPUTS as u32);
        start_transport(INPUTS as u32);
    }

    fn sweep_enter_parallel(&mut self, order: Option<&[u8]>) {
        configure_sequence(INPUTS as u32);
        start_transport(INPUTS as u32);
        if let Some(order) = order {
            self.all_inputs_on(order);
        }
    }

    #[inline(always)]
    fn trigger_after(&mut self, delay_us: u16) {
        let timer = timer();
        clear_bits(timer.ctl0(), TIMER_CTL0_CEN);
        write_bits(timer.cnt(), 0);
        write_bits(timer.ch1cv(), u32::from(delay_us));
        let adc = adc();
        clear_bits(adc.ctl1(), ADC_CTL1_ETSRC);
        set_bits(adc.ctl1(), ADC_CTL1_ETSRC_TIMER1_CH1);
        set_bits(timer.ctl0(), TIMER_CTL0_CEN);
    }

    fn trigger_at(&mut self, ticks: u16) {
        let compare = ticks.wrapping_sub(1);
        let timer = timer();
        clear_bits(timer.ctl0(), TIMER_CTL0_CEN);
        let counter = read_bits(timer.cnt());
        let flags = read_bits(timer.intf());
        if flags & (TIMER_INTF_UPIF | TIMER_INTF_CH1IF) != 0 || compare <= counter as u16 {
            write_bits(timer.cnt(), u32::from(compare).wrapping_sub(1));
        }
        write_bits(timer.ch1cv(), u32::from(compare));
        modify_bits(adc().ctl1(), |bits| {
            (bits & !ADC_CTL1_ETSRC) | ADC_CTL1_ETSRC_TIMER1_CH1
        });
        set_bits(timer.ctl0(), TIMER_CTL0_CEN);
    }

    fn end_cycle(&mut self, period_us: u16) {
        let adc = adc();
        clear_bits(adc.ctl1(), ADC_CTL1_ETSRC);
        set_bits(adc.ctl1(), ADC_CTL1_ETSRC_TIMER0_CH0);
        let timer = timer();
        write_bits(timer.intf(), !TIMER_INTF_CH1IF);
        write_bits(timer.intf(), !TIMER_INTF_UPIF);
        write_bits(timer.cnt(), 0);
        write_bits(timer.ch1cv(), u32::from(period_us));
        set_bits(timer.ctl0(), TIMER_CTL0_CEN);
    }

    #[inline(always)]
    fn micros(&self) -> u16 {
        microseconds()
    }
}

fn address_values(pins: [u8; ADDRESS_LINES]) -> [u32; ADDRESSES] {
    core::array::from_fn(|address| {
        (0..ADDRESS_LINES).zip(pins).fold(0, |value, (line, pin)| {
            let mask = 1_u32 << pin;
            value
                | if (address >> line) & 1 == 1 {
                    mask
                } else {
                    mask << RESET_SHIFT
                }
        })
    })
}

const fn sample_time_bits(sample_time: SampleTime) -> u32 {
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

fn configure_output(line: Line, high: bool) {
    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), 1 << line.port));
    let shift = 2 * u32::from(line.pin);
    critical_section::with(|_| {
        let port = gpio(line.port);
        modify_bits(port.ctl(), |bits| {
            (bits & !(0b11 << shift)) | GPIO_MODE_OUTPUT << shift
        });
        modify_bits(port.pud(), |bits| bits & !(0b11 << shift));
        clear_bits(port.omode(), line.mask());
        modify_bits(port.ospd(), |bits| {
            (bits & !(0b11 << shift)) | GPIO_SPEED_50MHZ << shift
        });
    });
    if high {
        write_bits(gpio(line.port).bop(), line.mask());
    } else {
        write_bits(gpio(line.port).bc(), line.mask());
    }
}

fn configure_analog(line: Line) {
    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), 1 << line.port));
    let shift = 2 * u32::from(line.pin);
    critical_section::with(|_| {
        let port = gpio(line.port);
        modify_bits(port.ctl(), |bits| {
            (bits & !(0b11 << shift)) | GPIO_MODE_ANALOG << shift
        });
        modify_bits(port.pud(), |bits| bits & !(0b11 << shift));
    });
}

fn configure_trigger(clocks: &Clocks) {
    rcu::with_registers(|rcu| {
        set_bits(rcu.apb1en(), APB1EN_TIMER1EN);
        set_bits(rcu.apb1rst(), APB1RST_TIMER1RST);
        clear_bits(rcu.apb1rst(), APB1RST_TIMER1RST);
    });
    let timer = timer();
    write_bits(timer.psc(), crate::time::prescaler(clocks.apb1_timers));
    clear_bits(timer.ctl0(), TIMER_CTL0_DIR_CAM);
    write_bits(timer.car(), TIMER_PERIOD);
    clear_bits(timer.ctl0(), TIMER_CTL0_CKDIV);
    write_bits(timer.swevg(), TIMER_SWEVG_UPG);
    clear_bits(timer.ctl0(), TIMER_CTL0_ARSE);
    clear_bits(timer.chctl2(), TIMER_CHCTL2_CH1EN);
    clear_bits(timer.chctl0_output(), TIMER_CHCTL0_CH1MS);
    set_bits(timer.chctl2(), TIMER_CHCTL2_CH1EN);
    clear_bits(timer.chctl2(), TIMER_CHCTL2_CH1P);
    modify_bits(timer.chctl0_output(), |bits| {
        (bits & !TIMER_CHCTL0_CH1COMCTL) | TIMER_CHCTL0_CH1COMCTL_PWM1
    });
    clear_bits(timer.chctl0_output(), TIMER_CHCTL0_CH1COMSEN);
    set_bits(timer.ctl0(), TIMER_CTL0_SPM);
}

fn configure_sequence(length: u32) {
    let adc = adc();
    let last = length.saturating_sub(1);
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
}

fn start_transport(count: u32) {
    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), AHB1EN_DMA1EN));
    let dma = dma();
    clear_bits(dma.ch0ctl(), DMA_CHCTL_CHEN);
    write_bits(dma.ch0ctl(), 0);
    write_bits(dma.ch0cnt(), 0);
    write_bits(dma.ch0paddr(), 0);
    write_bits(dma.ch0m0addr(), 0);
    write_bits(dma.ch0m1addr(), 0);
    write_bits(dma.ch0fctl(), 0);
    write_bits(dma.intc0(), DMA_CHANNEL0_FLAGS);
    clear_bits(dma.ch0fctl(), DMA_CHFCTL_MDMEN);
    write_bits(dma.ch0paddr(), adc().rdata().as_ptr().addr() as u32);
    write_bits(dma.ch0m0addr(), SAMPLES.as_ptr().addr() as u32);
    write_bits(dma.ch0cnt(), count);
    modify_bits(dma.ch0ctl(), |bits| {
        (bits & !(DMA_CHCTL_PWIDTH | DMA_CHCTL_MWIDTH | DMA_CHCTL_PRIO | DMA_CHCTL_TM))
            | DMA_CHCTL_PWIDTH_32
            | DMA_CHCTL_MWIDTH_32
            | DMA_CHCTL_PRIO_HIGH
    });
    clear_bits(dma.ch0ctl(), DMA_CHCTL_PNAGA);
    set_bits(dma.ch0ctl(), DMA_CHCTL_MNAGA);
    clear_bits(dma.ch0ctl(), DMA_CHCTL_CMEN);
    clear_bits(dma.ch0ctl(), DMA_CHCTL_PERIEN);
    set_bits(dma.ch0ctl(), DMA_CHCTL_CMEN);
    set_bits(dma.ch0ctl(), DMA_CHCTL_CHEN);
}

fn wait_microseconds(duration: u16) {
    let start = microseconds();
    while microseconds().wrapping_sub(start) < duration {}
}

#[inline(always)]
fn microseconds() -> u16 {
    // SAFETY: a read of the counter of TIMER4, which the time driver owns; reading it changes nothing.
    read_bits(unsafe { &*pac::Timer4::ptr() }.cnt()) as u16
}

#[inline(always)]
fn adc() -> &'static pac::adc0::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Adc0`; from then on only this module touches its
    // registers, from the scan interrupt or between cycles, which never overlap.
    unsafe { &*pac::Adc0::ptr() }
}

fn adc_common() -> &'static pac::adc_common::RegisterBlock {
    // SAFETY: `Converter::new` consumed `AdcCommon`; only its set-up writes these registers.
    unsafe { &*pac::AdcCommon::ptr() }
}

#[inline(always)]
fn timer() -> &'static pac::timer1::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Timer1`; only the scan uses it, from the scan interrupt
    // or between cycles, which never overlap.
    unsafe { &*pac::Timer1::ptr() }
}

fn dma() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: channel 0 of DMA1 belongs to the scan (other drivers use other channels of the
    // controller); the scan changes it only between cycles, and its writes to the shared flag
    // clear register carry the bits of channel 0 alone.
    unsafe { &*pac::Dma1::ptr() }
}

#[inline(always)]
fn gpio(port: u8) -> &'static pac::gpioa::RegisterBlock {
    let address = pac::Gpioa::PTR as usize + usize::from(port) * PORT_STRIDE;
    // SAFETY: every port of the family has this register block at `GPIOA + n * 0x400`, `Line`
    // only holds ports A to I, and the scan writes only the set/reset registers of its own pins
    // outside critical sections; mode registers change inside critical sections.
    unsafe { &*(address as *const pac::gpioa::RegisterBlock) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_values_set_the_one_bits_and_reset_the_others() {
        let values = address_values([8, 9, 10, 11]);
        assert_eq!(values[0], 0x0F00 << 16);
        assert_eq!(values[5], 0x0500 | 0x0A00 << 16);
        assert_eq!(values[15], 0x0F00);
    }
}
