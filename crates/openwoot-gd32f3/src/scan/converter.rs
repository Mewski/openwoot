#![allow(
    clippy::cast_possible_truncation,
    clippy::inline_always,
    reason = "port letters, input counts and timer counts fit their narrower types on this \
              family, the timer counter is compared as its low 16 bits as in the C HAL, and the \
              per-sample steps are inlined into the scan interrupt as the C HAL's always_inline \
              helpers"
)]

use cortex_m::peripheral::NVIC;
use openwoot::scan::{self, Sample};

use super::{
    Conversion, InterruptHandler, Peripherals, Wiring,
    adc::{self, MAX_RANKS, Trigger},
    transport, trigger,
    wiring::ADDRESS_LINES,
};
use crate::{
    gpio::{Level, Levels, Line},
    interrupt::{Binding, typelevel},
    pac,
    rcu::Clocks,
    time,
};

const ADDRESSES: usize = 1 << ADDRESS_LINES;

#[derive(Debug)]
pub struct Converter<const INPUTS: usize> {
    address_levels: [Levels; ADDRESSES],
    enable: [Line; INPUTS],
    channels: [u8; INPUTS],
    reference_channel: u8,
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
            address_levels: address_levels(address_port, wiring.address_pins),
            enable: wiring.enable,
            channels: wiring.inputs.map(|input| input.channel),
            reference_channel: wiring.reference.channel,
            sample_time: adc::sample_time_bits(wiring.sample_time),
        };
        for pin in wiring.address_pins {
            Line::from_index(address_port, pin).into_output(Level::Low);
        }
        for line in wiring.enable {
            line.into_output(Level::High);
        }
        trigger::configure(clocks);
        transport::start(1);
        for input in wiring.inputs {
            input.line.into_analog();
        }
        wiring.reference.line.into_analog();
        adc::configure(
            INPUTS,
            converter.reference_channel,
            converter.sample_time,
            wiring.oversampling,
        );
        converter
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
        adc::convert_at_rank(rank, channel, self.sample_time);
    }

    fn reference_sequence(&self, length: usize) {
        adc::configure_reference_sequence(length, self.reference_channel, self.sample_time);
    }

    fn all_inputs_off(&self) {
        for line in self.enable {
            line.set_high();
        }
    }

    fn all_inputs_on(&self, order: &[u8]) {
        for (rank, &input) in (0_u8..).zip(order) {
            self.convert_at_rank(rank, input);
            if let Some(line) = self.enable_line(input) {
                line.set_low();
            }
        }
    }
}

impl<const INPUTS: usize> scan::Converter for Converter<INPUTS> {
    #[inline(always)]
    fn acknowledge() {
        adc::acknowledge();
    }

    #[inline(always)]
    fn request_interrupt() {
        NVIC::pend(pac::Interrupt::ADC0_1);
    }

    #[inline(always)]
    fn sample(&self, rank: usize) -> Sample {
        transport::sample(rank)
    }

    #[inline(always)]
    fn select_address(&mut self, address: u8) {
        if let Some(levels) = self.address_levels.get(usize::from(address)) {
            levels.apply();
        }
    }

    #[inline(always)]
    fn select_input(&mut self, previous: u8, next: u8) {
        if let Some(line) = self.enable_line(previous) {
            line.set_high();
        }
        if let Some(line) = self.enable_line(next) {
            line.set_low();
        }
        self.convert_at_rank(0, next);
    }

    fn enter_single(&mut self, previous: u8) {
        self.all_inputs_off();
        if let Some(line) = self.enable_line(previous) {
            line.set_high();
        }
        if let Some(line) = self.enable_line(0) {
            line.set_low();
        }
        self.convert_at_rank(0, 0);
        self.select_address(0);
        adc::configure_sequence(1);
        self.reference_sequence(1);
        transport::start(1);
    }

    fn enter_parallel(&mut self, order: &[u8], address: u8) {
        self.all_inputs_off();
        self.all_inputs_on(order);
        self.select_address(address);
        adc::configure_sequence(INPUTS);
        self.reference_sequence(INPUTS);
        transport::start(INPUTS as u32);
    }

    fn sweep_enter_parallel(&mut self, order: Option<&[u8]>) {
        transport::stop();
        adc::configure_sequence(INPUTS);
        transport::start(INPUTS as u32);
        if let Some(order) = order {
            self.all_inputs_on(order);
        }
    }

    #[inline(always)]
    fn trigger_after(&mut self, delay_us: u16) {
        trigger::stop();
        trigger::set_counter(0);
        trigger::set_compare(u32::from(delay_us));
        adc::clear_trigger();
        adc::set_trigger(Trigger::Timer);
        trigger::start();
    }

    fn trigger_at(&mut self, ticks: u16) {
        let compare = ticks.wrapping_sub(1);
        trigger::stop();
        let counter = trigger::counter();
        let fired = trigger::fired();
        if fired || compare <= counter as u16 {
            trigger::set_counter(u32::from(compare).wrapping_sub(1));
        }
        trigger::set_compare(u32::from(compare));
        adc::select_trigger(Trigger::Timer);
        trigger::start();
    }

    fn end_cycle(&mut self, period_us: u16) {
        adc::clear_trigger();
        adc::set_trigger(Trigger::Software);
        trigger::clear_flags();
        trigger::set_counter(0);
        trigger::set_compare(u32::from(period_us));
        trigger::start();
    }

    fn recover(&mut self) {
        transport::restart();
    }

    #[inline(always)]
    fn micros(&self) -> u16 {
        time::microseconds()
    }
}

#[must_use]
pub const fn overruns() -> Option<u32> {
    None
}

fn address_levels(port: u8, pins: [u8; ADDRESS_LINES]) -> [Levels; ADDRESSES] {
    core::array::from_fn(|address| {
        Levels::new(
            port,
            (0..ADDRESS_LINES).zip(pins).map(|(line, pin)| {
                let level = if (address >> line) & 1 == 1 {
                    Level::High
                } else {
                    Level::Low
                };
                (pin, level)
            }),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_levels_drive_each_address_bit_onto_its_pin() {
        let levels = address_levels(1, [0, 1, 10, 11]);
        let pins =
            |bits: [Level; ADDRESS_LINES]| Levels::new(1, [0, 1, 10, 11].into_iter().zip(bits));
        assert_eq!(levels[0], pins([Level::Low; ADDRESS_LINES]));
        assert_eq!(
            levels[6],
            pins([Level::Low, Level::High, Level::High, Level::Low])
        );
        assert_eq!(levels[15], pins([Level::High; ADDRESS_LINES]));
    }
}
