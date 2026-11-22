use crate::{
    pac, rcu,
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

pub(crate) const ENABLE: u32 = 1 << 0;
pub(crate) const FULL_TRANSFER_INTERRUPT: u32 = 1 << 1;
pub(crate) const TRANSFER_ERROR_INTERRUPT: u32 = 1 << 3;
pub(crate) const INTERRUPTS: u32 = FULL_TRANSFER_INTERRUPT | TRANSFER_ERROR_INTERRUPT;
pub(crate) const MEMORY_TO_PERIPHERAL: u32 = 1 << 4;
pub(crate) const CIRCULAR: u32 = 1 << 5;
pub(crate) const PERIPHERAL_INCREMENT: u32 = 1 << 6;
pub(crate) const MEMORY_INCREMENT: u32 = 1 << 7;
pub(crate) const PERIPHERAL_WIDTH: u32 = 0b11 << 8;
pub(crate) const PERIPHERAL_WIDTH_32: u32 = 0b10 << 8;
pub(crate) const MEMORY_WIDTH: u32 = 0b11 << 10;
pub(crate) const MEMORY_WIDTH_32: u32 = 0b10 << 10;
pub(crate) const PRIORITY: u32 = 0b11 << 12;
pub(crate) const PRIORITY_LOW: u32 = 0;
pub(crate) const PRIORITY_ULTRA_HIGH: u32 = 0b11 << 12;
pub(crate) const FLAG_FULL_TRANSFER: u32 = 1 << 1;
pub(crate) const FLAG_TRANSFER_ERROR: u32 = 1 << 3;
pub(crate) const ALL_FLAGS: u32 = 0b1111;

const AHBEN_DMA0EN: u32 = 1 << 0;
const FLAG_BITS_PER_CHANNEL: u32 = 4;

#[derive(Debug)]
pub(crate) struct Channel<const CONTROLLER: u8, const INDEX: u8>;

impl<const CONTROLLER: u8, const INDEX: u8> Channel<CONTROLLER, INDEX> {
    pub(crate) const fn new() -> Self {
        Self
    }

    const fn flag_shift() -> u32 {
        FLAG_BITS_PER_CHANNEL * INDEX as u32
    }
}

macro_rules! channels {
    ($($index:literal: $control:ident, $count:ident, $peripheral:ident, $memory:ident;)*) => {
        $(
            #[expect(
                dead_code,
                reason = "every channel offers the full register set, and each driver uses the \
                          accesses it needs"
            )]
            impl Channel<0, $index> {
                pub(crate) fn enable_clock(&self) {
                    rcu::with_registers(|rcu| set_bits(rcu.ahben(), AHBEN_DMA0EN));
                }

                pub(crate) fn control(&self) -> u32 {
                    read_bits(dma0().$control())
                }

                pub(crate) fn write_control(&self, value: u32) {
                    write_bits(dma0().$control(), value);
                }

                pub(crate) fn modify_control(&self, change: impl FnOnce(u32) -> u32) {
                    modify_bits(dma0().$control(), change);
                }

                pub(crate) fn set_control(&self, mask: u32) {
                    set_bits(dma0().$control(), mask);
                }

                pub(crate) fn clear_control(&self, mask: u32) {
                    clear_bits(dma0().$control(), mask);
                }

                pub(crate) fn set_peripheral_address(&self, address: u32) {
                    write_bits(dma0().$peripheral(), address);
                }

                pub(crate) fn set_memory_address(&self, address: u32) {
                    write_bits(dma0().$memory(), address);
                }

                pub(crate) fn set_count(&self, count: u32) {
                    write_bits(dma0().$count(), count);
                }

                pub(crate) fn flags(&self) -> u32 {
                    (read_bits(dma0().intf()) >> Self::flag_shift()) & ALL_FLAGS
                }

                pub(crate) fn clear_flags(&self) {
                    write_bits(dma0().intc(), ALL_FLAGS << Self::flag_shift());
                }
            }
        )*
    };
}

channels! {
    0: ch0ctl, ch0cnt, ch0paddr, ch0maddr;
    3: ch3ctl, ch3cnt, ch3paddr, ch3maddr;
    4: ch4ctl, ch4cnt, ch4paddr, ch4maddr;
    5: ch5ctl, ch5cnt, ch5paddr, ch5maddr;
}

fn dma0() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: the controller is shared by channel owners that never touch each other's channel:
    // channel 0 belongs to the scan transport, channels 3 and 4 to the SPI transfer, channel 5 to
    // the I2C transfer. Each `Channel` accesses only its own channel registers; the flag clear
    // register is write-one-to-clear and every write carries only the channel's own bits, and the
    // shared flag register is only read.
    unsafe { &*pac::Dma0::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_flags_lie_four_bits_apart() {
        assert_eq!(ALL_FLAGS << Channel::<0, 0>::flag_shift(), 0x000F);
        assert_eq!(ALL_FLAGS << Channel::<0, 3>::flag_shift(), 0xF000);
        assert_eq!(ALL_FLAGS << Channel::<0, 5>::flag_shift(), 0x00F0_0000);
    }
}
