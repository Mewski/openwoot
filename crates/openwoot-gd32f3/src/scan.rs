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

const AHBEN_DMA0EN: u32 = 1 << 0;
const APB1EN_TIMER1EN: u32 = 1 << 0;
const APB1RST_TIMER1RST: u32 = 1 << 0;
const APB2EN_PORT_SHIFT: u8 = 2;
const APB2EN_ADC0EN: u32 = 1 << 9;
const APB2EN_ADC1EN: u32 = 1 << 10;
const CFG0_ADCPSC: u32 = 0b11 << 14;
const CFG0_ADCPSC_APB2_DIV4: u32 = 0b01 << 14;
const CFG0_ADCPSC_2: u32 = 1 << 28;
const CFG1_ADCPSC_3: u32 = 1 << 29;

const GPIO_OUTPUT_PUSH_PULL_50MHZ: u32 = 0b0011;
const GPIO_INPUT_ANALOG: u32 = 0b0000;

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

const DMA_CHCTL_CHEN: u32 = 1 << 0;
const DMA_CHCTL_DIR: u32 = 1 << 4;
const DMA_CHCTL_CMEN: u32 = 1 << 5;
const DMA_CHCTL_PNAGA: u32 = 1 << 6;
const DMA_CHCTL_MNAGA: u32 = 1 << 7;
const DMA_CHCTL_PWIDTH: u32 = 0b11 << 8;
const DMA_CHCTL_PWIDTH_32: u32 = 0b10 << 8;
const DMA_CHCTL_MWIDTH: u32 = 0b11 << 10;
const DMA_CHCTL_MWIDTH_32: u32 = 0b10 << 10;
const DMA_CHCTL_PRIO: u32 = 0b11 << 12;
const DMA_CHCTL_PRIO_HIGH: u32 = 0b10 << 12;
const DMA_CHANNEL0_FLAGS: u32 = 0xF;

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
            port >= 'A' && port <= 'G' && pin < 16,
            "a pin of ports A to G"
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
pub struct Wiring<const INPUTS: usize> {
    pub address_port: char,
    pub address_pins: [u8; ADDRESS_LINES],
    pub enable: [Line; INPUTS],
    pub inputs: [AnalogInput; INPUTS],
    pub reference: AnalogInput,
    pub sample_time: SampleTime,
    pub oversampling: Oversampling,
}

#[derive(Debug)]
pub struct Peripherals {
    pub adc: pac::Adc0,
    pub reference_adc: pac::Adc1,
    pub timer: pac::Timer1,
}

pub trait Conversion {
    fn on_conversion();
}

#[derive(Debug)]
pub struct InterruptHandler<Hook>(PhantomData<Hook>);

impl<Hook: Conversion> Handler<typelevel::ADC0_1> for InterruptHandler<Hook> {
    // SAFETY: the handler runs only in the ADC0_1 interrupt, as `Handler` requires; the hook it
    // calls is safe code.
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
    reference_channel: u8,
    sample_time: u32,
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
        _irqs: impl Binding<typelevel::ADC0_1, InterruptHandler<Hook>>,
    ) -> Self {
        const { assert!(INPUTS > 0 && INPUTS <= MAX_RANKS) };
        let Peripherals {
            adc: _,
            reference_adc: _,
            timer: _,
        } = peripherals;
        let address_port = wiring.address_port as u8 - b'A';
        let converter = Self {
            address_port,
            address_values: address_values(wiring.address_pins),
            enable: wiring.enable,
            channels: wiring.inputs.map(|input| input.channel),
            reference_channel: wiring.reference.channel,
            sample_time: sample_time_bits(wiring.sample_time),
        };
        converter.configure_outputs(wiring);
        configure_trigger(clocks);
        converter.configure_converter(wiring);
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

    fn configure_converter(&self, wiring: &Wiring<INPUTS>) {
        start_transport(1);
        for input in wiring.inputs {
            configure_analog(input.line);
        }
        configure_analog(wiring.reference.line);
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
        for rank in 0..INPUTS as u8 {
            convert_at_rank!(reference, rank, self.reference_channel, self.sample_time);
        }
        modify_bits(adc.ctl1(), |bits| {
            (bits & !ADC_CTL1_ETSRC) | ADC_CTL1_ETSRC_TIMER1_CH1
        });
        modify_bits(reference.ctl1(), |bits| {
            (bits & !ADC_CTL1_ETSRC) | ADC_CTL1_ETSRC_SOFTWARE
        });
        set_bits(adc.ctl1(), ADC_CTL1_ETERC);
        set_bits(reference.ctl1(), ADC_CTL1_ETERC);
        let shift = oversampling_shift(wiring.oversampling);
        configure_oversampling!(adc, shift);
        configure_oversampling!(reference, shift);
        interrupt::enable(pac::Interrupt::ADC0_1, Priority::SCAN);
        set_bits(adc.ctl0(), ADC_CTL0_EOCIE);
        enable_and_calibrate!(adc);
        enable_and_calibrate!(reference);
        set_bits(adc.ctl1(), ADC_CTL1_DMA);
    }

    #[inline(always)]
    fn enable_line(&self, input: u8) -> Option<Line> {
        self.enable.get(usize::from(input)).copied()
    }

    #[inline(always)]
    fn convert_at_rank(&self, rank: u8, input: u8) {
        if let Some(&channel) = self.channels.get(usize::from(input)) {
            convert_at_rank!(adc(), rank, channel, self.sample_time);
        }
    }

    fn reference_sequence(&self, length: usize) {
        let reference = reference_adc();
        configure_sequence!(reference, length);
        for rank in 0..length as u8 {
            convert_at_rank!(reference, rank, self.reference_channel, self.sample_time);
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
        NVIC::pend(pac::Interrupt::ADC0_1);
    }

    #[inline(always)]
    fn sample(&self, rank: usize) -> Sample {
        let word = SAMPLES
            .get(rank)
            .map_or(0, |sample| sample.load(Ordering::Relaxed));
        Sample {
            reading: word as u16,
            reference: (word >> 16) as u16,
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
        configure_sequence!(adc(), 1);
        self.reference_sequence(1);
        start_transport(1);
    }

    fn enter_parallel(&mut self, order: &[u8], address: u8) {
        self.all_inputs_off();
        self.all_inputs_on(order);
        self.select_address(address);
        configure_sequence!(adc(), INPUTS);
        self.reference_sequence(INPUTS);
        start_transport(INPUTS as u32);
    }

    fn sweep_enter_parallel(&mut self, order: Option<&[u8]>) {
        clear_bits(dma().ch0ctl(), DMA_CHCTL_CHEN);
        configure_sequence!(adc(), INPUTS);
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
        set_bits(adc.ctl1(), ADC_CTL1_ETSRC_SOFTWARE);
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

fn configure_pin(line: Line, nibble: u32) {
    rcu::with_registers(|rcu| set_bits(rcu.apb2en(), 1 << (line.port + APB2EN_PORT_SHIFT)));
    critical_section::with(|_| {
        let port = gpio(line.port);
        let field = |index: u8| {
            let shift = 4 * u32::from(index);
            move |bits: u32| (bits & !(0b1111 << shift)) | nibble << shift
        };
        if line.pin < 8 {
            modify_bits(port.ctl0(), field(line.pin));
        } else {
            modify_bits(port.ctl1(), field(line.pin - 8));
        }
    });
}

fn configure_output(line: Line, high: bool) {
    configure_pin(line, GPIO_OUTPUT_PUSH_PULL_50MHZ);
    if high {
        write_bits(gpio(line.port).bop(), line.mask());
    } else {
        write_bits(gpio(line.port).bc(), line.mask());
    }
}

fn configure_analog(line: Line) {
    configure_pin(line, GPIO_INPUT_ANALOG);
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

fn start_transport(count: u32) {
    rcu::with_registers(|rcu| set_bits(rcu.ahben(), AHBEN_DMA0EN));
    let dma = dma();
    clear_bits(dma.ch0ctl(), DMA_CHCTL_CHEN);
    write_bits(dma.ch0ctl(), 0);
    write_bits(dma.ch0cnt(), 0);
    write_bits(dma.ch0paddr(), 0);
    write_bits(dma.ch0maddr(), 0);
    write_bits(dma.intc(), DMA_CHANNEL0_FLAGS);
    write_bits(dma.ch0paddr(), adc().rdata().as_ptr().addr() as u32);
    write_bits(dma.ch0maddr(), SAMPLES.as_ptr().addr() as u32);
    write_bits(dma.ch0cnt(), count);
    modify_bits(dma.ch0ctl(), |bits| {
        (bits & !(DMA_CHCTL_PWIDTH | DMA_CHCTL_MWIDTH | DMA_CHCTL_PRIO))
            | DMA_CHCTL_PWIDTH_32
            | DMA_CHCTL_MWIDTH_32
            | DMA_CHCTL_PRIO_HIGH
    });
    clear_bits(dma.ch0ctl(), DMA_CHCTL_PNAGA);
    set_bits(dma.ch0ctl(), DMA_CHCTL_MNAGA);
    clear_bits(dma.ch0ctl(), DMA_CHCTL_DIR);
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

fn reference_adc() -> &'static pac::adc1::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Adc1`; only the scan configures it, between cycles.
    unsafe { &*pac::Adc1::ptr() }
}

#[inline(always)]
fn timer() -> &'static pac::timer1::RegisterBlock {
    // SAFETY: `Converter::new` consumed `Timer1`; only the scan uses it, from the scan interrupt
    // or between cycles, which never overlap.
    unsafe { &*pac::Timer1::ptr() }
}

fn dma() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: channel 0 of DMA0 belongs to the scan (other drivers use other channels of the
    // controller); the scan changes it only between cycles, and its writes to the shared flag
    // clear register carry the bits of channel 0 alone.
    unsafe { &*pac::Dma0::ptr() }
}

#[inline(always)]
fn gpio(port: u8) -> &'static pac::gpioa::RegisterBlock {
    let address = pac::Gpioa::PTR as usize + usize::from(port) * PORT_STRIDE;
    // SAFETY: every port of the family has this register block at `GPIOA + n * 0x400`, `Line`
    // only holds ports A to G, and the scan writes only the set/reset registers of its own pins
    // outside critical sections; mode registers change inside critical sections.
    unsafe { &*(address as *const pac::gpioa::RegisterBlock) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_values_set_the_one_bits_and_reset_the_others() {
        let values = address_values([0, 1, 10, 11]);
        assert_eq!(values[0], 0x0C03 << 16);
        assert_eq!(values[6], 0x0402 | 0x0801 << 16);
        assert_eq!(values[15], 0x0C03);
    }

    #[test]
    fn eight_times_oversampling_shifts_three_bits() {
        assert_eq!(oversampling_shift(Oversampling::Times8), 3);
    }
}
