#![allow(
    clippy::inline_always,
    reason = "the port accessor runs inside the scan interrupt as the C HAL's always_inline helpers"
)]

use super::{Level, OutputType};
use crate::{
    pac, rcu,
    register::{modify_bits, set_bits},
};

pub(super) const MODE_INPUT: u32 = 0b00;
pub(super) const MODE_OUTPUT: u32 = 0b01;
pub(super) const MODE_ALTERNATE: u32 = 0b10;
pub(super) const MODE_ANALOG: u32 = 0b11;

const PORT_STRIDE: usize = 0x400;
const RESET_SHIFT: u8 = 16;

#[inline(always)]
pub(super) fn port(index: u8) -> &'static pac::gpioa::RegisterBlock {
    let address = pac::Gpioa::PTR as usize + port_offset(index);
    // SAFETY: every port of the family has this register block at `GPIOA + n * 0x400`; a `Pin`
    // exists only for a port whose `Port` was built from its PAC peripheral, and a `Line` only
    // holds ports A to I. Registers that several pins share are changed only inside critical
    // sections; output levels are written through the set/reset and bit-clear registers, which
    // change only the bits written as one.
    unsafe { &*(address as *const pac::gpioa::RegisterBlock) }
}

pub(super) const fn port_index(port: char) -> u8 {
    port as u8 - b'A'
}

pub(super) fn enable_clock(index: u8) {
    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), 1 << index));
}

pub(super) fn modify_two_bit_field<Spec>(register: &pac::generic::Reg<Spec>, index: u8, value: u32)
where
    Spec: pac::generic::RegisterSpec<Ux = u32> + pac::generic::Readable + pac::generic::Writable,
{
    modify_bits(register, |bits| two_bit_field(bits, index, value));
}

pub(super) fn modify_output_type(
    port: &pac::gpioa::RegisterBlock,
    index: u8,
    output_type: OutputType,
) {
    let open_drain = u32::from(output_type == OutputType::OpenDrain);
    modify_bits(port.omode(), |bits| {
        (bits & !(1 << index)) | open_drain << index
    });
}

pub(super) const fn set_reset(index: u8, level: Level) -> u32 {
    match level {
        Level::High => 1 << index,
        Level::Low => 1 << (index + RESET_SHIFT),
    }
}

pub(super) const fn alternate_field(register: u32, index: u8, function: u8) -> u32 {
    let shift = index as u32 * 4;
    (register & !(0b1111 << shift)) | ((function as u32) << shift)
}

pub(super) const fn level_of(register: u32, index: u8) -> Level {
    if register & (1 << index) == 0 {
        Level::Low
    } else {
        Level::High
    }
}

#[inline(always)]
const fn port_offset(index: u8) -> usize {
    index as usize * PORT_STRIDE
}

const fn two_bit_field(register: u32, index: u8, value: u32) -> u32 {
    let shift = index as u32 * 2;
    (register & !(0b11 << shift)) | (value << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_carry_the_reference_manual_encodings() {
        assert_eq!(two_bit_field(0xFFFF_FFFF, 5, MODE_OUTPUT), 0xFFFF_F7FF);
        assert_eq!(two_bit_field(0, 15, MODE_ANALOG), 0xC000_0000);
        assert_eq!(alternate_field(0xFFFF_FFFF, 7, 10), 0xAFFF_FFFF);
        assert_eq!(alternate_field(0, 2, 4), 0x0000_0400);
    }

    #[test]
    fn ports_lie_one_kilobyte_apart() {
        assert_eq!(port_offset(port_index('A')), 0);
        assert_eq!(port_offset(port_index('D')), 0xC00);
    }

    #[test]
    fn levels_are_read_from_the_pin_bit() {
        assert_eq!(level_of(0x0400, 10), Level::High);
        assert_eq!(level_of(0xFBFF, 10), Level::Low);
    }

    #[test]
    fn low_levels_are_written_through_the_reset_half() {
        assert_eq!(set_reset(3, Level::High), 0x0000_0008);
        assert_eq!(set_reset(3, Level::Low), 0x0008_0000);
    }
}
