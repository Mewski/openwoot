#![allow(
    clippy::inline_always,
    reason = "the port accessor is inlined into the scan interrupt as the C HAL's always_inline \
              helpers"
)]

use super::Level;
use crate::{
    pac, rcu,
    register::{modify_bits, set_bits},
};

pub(super) const INPUT_ANALOG: u32 = 0b00 << 2;
pub(super) const INPUT_FLOATING: u32 = 0b01 << 2;
pub(super) const INPUT_PULL: u32 = 0b10 << 2;
pub(super) const OUTPUT_PUSH_PULL: u32 = 0b00 << 2;
pub(super) const OUTPUT_OPEN_DRAIN: u32 = 0b01 << 2;
pub(super) const ALTERNATE_PUSH_PULL: u32 = 0b10 << 2;
pub(super) const ALTERNATE_OPEN_DRAIN: u32 = 0b11 << 2;

const PORT_STRIDE: usize = 0x400;
const RESET_SHIFT: u8 = 16;
const APB2EN_PORT_SHIFT: u8 = 2;
const PINS_PER_CONTROL: u8 = 8;

#[inline(always)]
pub(super) fn port(index: u8) -> &'static pac::gpioa::RegisterBlock {
    let address = pac::Gpioa::PTR as usize + port_offset(index);
    // SAFETY: every port of the family has this register block at `GPIOA + n * 0x400`, and the
    // gpio module reaches only ports A to G: a `Pin` exists only for a port whose `Port` was built
    // from its PAC peripheral, and a `Line` holds only ports A to G. Registers that several pins
    // share are changed only inside critical sections; output levels are written through the
    // set/reset and bit-clear registers, which change only the bits written as one.
    unsafe { &*(address as *const pac::gpioa::RegisterBlock) }
}

pub(super) const fn port_index(port: char) -> u8 {
    port as u8 - b'A'
}

pub(super) fn enable_clock(index: u8) {
    rcu::with_registers(|rcu| set_bits(rcu.apb2en(), 1 << (index + APB2EN_PORT_SHIFT)));
}

pub(super) fn set_mode(port: &pac::gpioa::RegisterBlock, pin: u8, nibble: u32) {
    if pin < PINS_PER_CONTROL {
        modify_bits(port.ctl0(), |bits| nibble_field(bits, pin, nibble));
    } else {
        modify_bits(port.ctl1(), |bits| {
            nibble_field(bits, pin - PINS_PER_CONTROL, nibble)
        });
    }
}

pub(super) const fn set_reset(pin: u8, level: Level) -> u32 {
    match level {
        Level::High => 1 << pin,
        Level::Low => 1 << (pin + RESET_SHIFT),
    }
}

pub(super) const fn level_of(register: u32, index: u8) -> Level {
    if register & (1 << index) == 0 {
        Level::Low
    } else {
        Level::High
    }
}

const fn port_offset(index: u8) -> usize {
    index as usize * PORT_STRIDE
}

const fn nibble_field(register: u32, index: u8, nibble: u32) -> u32 {
    let shift = index as u32 * 4;
    (register & !(0b1111 << shift)) | (nibble << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibbles_land_on_their_pin() {
        assert_eq!(nibble_field(0x4444_4444, 7, 0x3), 0x3444_4444);
        assert_eq!(nibble_field(0x4444_4444, 0, 0xF), 0x4444_444F);
    }

    #[test]
    fn ports_lie_one_kilobyte_apart() {
        assert_eq!(port_offset(port_index('A')), 0);
        assert_eq!(port_offset(port_index('C')), 0x800);
    }

    #[test]
    fn levels_are_read_from_the_pin_bit() {
        assert_eq!(level_of(0x8000, 15), Level::High);
        assert_eq!(level_of(0x7FFF, 15), Level::Low);
    }

    #[test]
    fn high_levels_set_and_low_levels_reset() {
        assert_eq!(set_reset(3, Level::High), 0x0008);
        assert_eq!(set_reset(3, Level::Low), 0x0008 << 16);
    }
}
