use crate::{
    pac, rcu,
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

pub(crate) const ENABLE: u32 = 1 << 0;
pub(crate) const SINGLE_DATA_ERROR_INTERRUPT: u32 = 1 << 1;
pub(crate) const TRANSFER_ERROR_INTERRUPT: u32 = 1 << 2;
pub(crate) const FULL_TRANSFER_INTERRUPT: u32 = 1 << 4;
pub(crate) const INTERRUPTS: u32 =
    SINGLE_DATA_ERROR_INTERRUPT | TRANSFER_ERROR_INTERRUPT | FULL_TRANSFER_INTERRUPT;
pub(crate) const DIRECTION: u32 = 0b11 << 6;
pub(crate) const MEMORY_TO_PERIPHERAL: u32 = 0b01 << 6;
pub(crate) const CIRCULAR: u32 = 1 << 8;
pub(crate) const PERIPHERAL_INCREMENT: u32 = 1 << 9;
pub(crate) const MEMORY_INCREMENT: u32 = 1 << 10;
pub(crate) const PERIPHERAL_WIDTH: u32 = 0b11 << 11;
pub(crate) const PERIPHERAL_WIDTH_32: u32 = 0b10 << 11;
pub(crate) const MEMORY_WIDTH: u32 = 0b11 << 13;
pub(crate) const MEMORY_WIDTH_32: u32 = 0b10 << 13;
pub(crate) const PRIORITY: u32 = 0b11 << 16;
pub(crate) const PRIORITY_LOW: u32 = 0;
pub(crate) const PRIORITY_ULTRA_HIGH: u32 = 0b11 << 16;
pub(crate) const REQUEST: u32 = 0b111 << REQUEST_SHIFT;
pub(crate) const FIFO_MULTI_DATA: u32 = 1 << 2;
pub(crate) const FIFO_RESET: u32 = 0x21;
pub(crate) const FLAG_SINGLE_DATA_ERROR: u32 = 1 << 2;
pub(crate) const FLAG_TRANSFER_ERROR: u32 = 1 << 3;
pub(crate) const FLAG_FULL_TRANSFER: u32 = 1 << 5;
pub(crate) const ALL_FLAGS: u32 = 0b11_1101;

const REQUEST_SHIFT: u32 = 25;
const AHB1EN_DMA0EN: u32 = 1 << 21;
const AHB1EN_DMA1EN: u32 = 1 << 22;

#[derive(Debug)]
pub(crate) struct Channel<const CONTROLLER: u8, const INDEX: u8>;

impl<const CONTROLLER: u8, const INDEX: u8> Channel<CONTROLLER, INDEX> {
    pub(crate) const fn new() -> Self {
        Self
    }
}

pub(crate) const fn request(number: u32) -> u32 {
    number << REQUEST_SHIFT
}

macro_rules! channels {
    ($(
        $controller:literal, $index:literal: $registers:ident, $clock:ident,
        $ctl:ident, $cnt:ident, $paddr:ident, $m0addr:ident, $m1addr:ident, $fctl:ident,
        $intf:ident, $intc:ident;
    )*) => {
        $(
            #[expect(
                dead_code,
                reason = "every channel offers the full register set, and each driver uses the \
                          accesses it needs"
            )]
            impl Channel<$controller, $index> {
                const FLAG_SHIFT: u32 = flag_shift($index);

                pub(crate) fn enable_clock(&self) {
                    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), $clock));
                }

                pub(crate) fn control(&self) -> u32 {
                    read_bits($registers().$ctl())
                }

                pub(crate) fn write_control(&self, value: u32) {
                    write_bits($registers().$ctl(), value);
                }

                pub(crate) fn modify_control(&self, change: impl FnOnce(u32) -> u32) {
                    modify_bits($registers().$ctl(), change);
                }

                pub(crate) fn set_control(&self, mask: u32) {
                    set_bits($registers().$ctl(), mask);
                }

                pub(crate) fn clear_control(&self, mask: u32) {
                    clear_bits($registers().$ctl(), mask);
                }

                pub(crate) fn set_peripheral_address(&self, address: u32) {
                    write_bits($registers().$paddr(), address);
                }

                pub(crate) fn set_memory_address(&self, address: u32) {
                    write_bits($registers().$m0addr(), address);
                }

                pub(crate) fn set_second_memory_address(&self, address: u32) {
                    write_bits($registers().$m1addr(), address);
                }

                pub(crate) fn set_count(&self, count: u32) {
                    write_bits($registers().$cnt(), count);
                }

                pub(crate) fn write_fifo_control(&self, value: u32) {
                    write_bits($registers().$fctl(), value);
                }

                pub(crate) fn clear_fifo_control(&self, mask: u32) {
                    clear_bits($registers().$fctl(), mask);
                }

                pub(crate) fn flags(&self) -> u32 {
                    (read_bits($registers().$intf()) >> Self::FLAG_SHIFT) & ALL_FLAGS
                }

                pub(crate) fn clear_flags(&self) {
                    write_bits($registers().$intc(), ALL_FLAGS << Self::FLAG_SHIFT);
                }
            }
        )*
    };
}

channels! {
    0, 6: dma0, AHB1EN_DMA0EN, ch6ctl, ch6cnt, ch6paddr, ch6m0addr, ch6m1addr, ch6fctl, intf1, intc1;
    1, 0: dma1, AHB1EN_DMA1EN, ch0ctl, ch0cnt, ch0paddr, ch0m0addr, ch0m1addr, ch0fctl, intf0, intc0;
    1, 2: dma1, AHB1EN_DMA1EN, ch2ctl, ch2cnt, ch2paddr, ch2m0addr, ch2m1addr, ch2fctl, intf0, intc0;
    1, 3: dma1, AHB1EN_DMA1EN, ch3ctl, ch3cnt, ch3paddr, ch3m0addr, ch3m1addr, ch3fctl, intf0, intc0;
}

const fn flag_shift(index: u8) -> u32 {
    match index % 4 {
        0 => 0,
        1 => 6,
        2 => 16,
        _ => 22,
    }
}

fn dma0() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: of DMA0 only channel 6 is used, by the LED bus (`i2c`); a `Channel` touches only its
    // own channel registers, and its writes to the shared write-one-to-clear flag register carry
    // only its own flag bits, so it never changes what another channel owns.
    unsafe { &*pac::Dma0::ptr() }
}

fn dma1() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: of DMA1 channel 0 belongs to the scan (`scan::transport`) and channels 2 and 3 to
    // `spi`; a `Channel` touches only its own channel registers, and its writes to the shared
    // write-one-to-clear flag register carry only its own flag bits, so it never changes what
    // another channel owns.
    unsafe { &*pac::Dma1::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_flags_sit_at_their_reference_manual_offsets() {
        assert_eq!(flag_shift(0), 0);
        assert_eq!(flag_shift(2), 16);
        assert_eq!(flag_shift(3), 22);
        assert_eq!(flag_shift(6), 16);
        assert_eq!(
            ALL_FLAGS << flag_shift(2) | ALL_FLAGS << flag_shift(3),
            0x0F7D_0000
        );
    }

    #[test]
    fn requests_select_the_sub_peripheral() {
        assert_eq!(request(1), 1 << 25);
        assert_eq!(request(3) & REQUEST, 3 << 25);
    }
}
