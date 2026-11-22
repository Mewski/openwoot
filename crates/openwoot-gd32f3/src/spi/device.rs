use core::convert::Infallible;

use embedded_hal::{digital::OutputPin, spi::ErrorType};
use embedded_hal_async::spi::{Operation, SpiBus, SpiDevice};

use super::{Error, Spi};
use crate::rcu::Hertz;

const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Debug)]
pub struct Device<ChipSelect> {
    bus: Spi,
    chip_select: ChipSelect,
    core_clock: Hertz,
}

struct Selected<'c, ChipSelect: OutputPin<Error = Infallible>> {
    chip_select: &'c mut ChipSelect,
}

impl<ChipSelect: OutputPin<Error = Infallible>> Device<ChipSelect> {
    #[must_use]
    pub fn new(bus: Spi, mut chip_select: ChipSelect, core_clock: Hertz) -> Self {
        let Ok(()) = chip_select.set_high();
        Self {
            bus,
            chip_select,
            core_clock,
        }
    }

    pub fn release(self) -> (Spi, ChipSelect) {
        (self.bus, self.chip_select)
    }
}

impl<ChipSelect: OutputPin<Error = Infallible>> ErrorType for Device<ChipSelect> {
    type Error = Error;
}

impl<ChipSelect: OutputPin<Error = Infallible>> SpiDevice for Device<ChipSelect> {
    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Error> {
        let selected = Selected::new(&mut self.chip_select);
        let mut result = Ok(());
        for operation in operations {
            result = match operation {
                Operation::Read(words) => self.bus.read(words).await,
                Operation::Write(words) => self.bus.write(words).await,
                Operation::Transfer(read, write) => self.bus.transfer(read, write).await,
                Operation::TransferInPlace(words) => self.bus.transfer_in_place(words).await,
                Operation::DelayNs(nanoseconds) => {
                    self.bus.idle();
                    delay(*nanoseconds, self.core_clock);
                    Ok(())
                }
            };
            if result.is_err() {
                break;
            }
        }
        self.bus.idle();
        drop(selected);
        result
    }
}

impl<'c, ChipSelect: OutputPin<Error = Infallible>> Selected<'c, ChipSelect> {
    fn new(chip_select: &'c mut ChipSelect) -> Self {
        let Ok(()) = chip_select.set_low();
        Self { chip_select }
    }
}

impl<ChipSelect: OutputPin<Error = Infallible>> Drop for Selected<'_, ChipSelect> {
    fn drop(&mut self) {
        let Ok(()) = self.chip_select.set_high();
    }
}

fn delay(nanoseconds: u32, core_clock: Hertz) {
    let cycles = u64::from(nanoseconds) * u64::from(core_clock.0) / NANOSECONDS_PER_SECOND;
    cortex_m::asm::delay(u32::try_from(cycles).unwrap_or(u32::MAX));
}
