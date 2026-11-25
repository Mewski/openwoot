mod bus_clear;
mod error;
mod pins;
mod registers;
mod step;
mod timing;
mod transfer;

use core::hint::spin_loop;

use bus_clear::Pins;
use embassy_futures::yield_now;
use embedded_hal_async::i2c::{
    self as hal_i2c, ErrorType, NoAcknowledgeSource, Operation, SevenBitAddress,
};
pub use error::Error;
use openwoot::diagnostics::{Bus, Transfers};
pub use pins::{ClockPin, DataPin};
use registers::{
    APB1EN_I2C0EN, CTL0_ACKEN, CTL0_I2CEN, CTL0_SMBEN, CTL0_SRESET, CTL0_START, CTL0_STOP,
    CTL1_DMAON, OWN_ADDRESS, STAT0_ADDSEND, STAT0_BTC, STAT0_SBSEND, STAT1_I2CBSY, STAT1_MASTER,
    modify_ctl1, registers, with_clock,
};
use step::step;
use timing::{Timing, timing};
pub use transfer::InterruptHandler;
use transfer::{Channel, Transfer};

use crate::{
    dma,
    interrupt::{self, Binding, Priority, typelevel},
    pac,
    rcu::{self, Clocks},
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

const MAX_TRANSFER: usize = 443;
const MAX_WRITES: usize = 4;
const RELEASE_SPINS: u32 = 10_000;

static TRANSFERS: Transfers = Transfers::new();

#[derive(Debug)]
pub struct I2c {
    peripheral: pac::I2c0,
    timing: Timing,
    pins: Pins,
}

impl I2c {
    #[must_use]
    pub fn new(
        i2c: pac::I2c0,
        clock: impl ClockPin,
        data: impl DataPin,
        _irqs: impl Binding<typelevel::I2C0_EV, InterruptHandler>
        + Binding<typelevel::I2C0_ER, InterruptHandler>
        + Binding<typelevel::DMA0_Channel6, InterruptHandler>,
        clocks: &Clocks,
    ) -> Self {
        rcu::with_registers(|rcu| set_bits(rcu.apb1en(), APB1EN_I2C0EN));
        Channel::new().enable_clock();
        let bus = Self {
            peripheral: i2c,
            timing: timing(clocks.apb1),
            pins: Pins::new(clock.into_line(), data.into_line(), clocks.system),
        };
        bus.pins.release();
        bus.configure();
        for irq in [
            pac::Interrupt::I2C0_EV,
            pac::Interrupt::I2C0_ER,
            pac::Interrupt::DMA0_Channel6,
        ] {
            interrupt::enable(irq, Priority::HIGHEST);
        }
        bus
    }

    fn configure(&self) {
        let i2c = &self.peripheral;
        critical_section::with(|section| {
            modify_ctl1(section, i2c, |bits| with_clock(bits, self.timing.clock_mhz));
        });
        write_bits(i2c.rt(), self.timing.rise_time);
        write_bits(i2c.ckcfg(), self.timing.clock_configuration);
        write_bits(i2c.saddr0(), OWN_ADDRESS);
        clear_bits(i2c.ctl0(), CTL0_SMBEN);
        set_bits(i2c.ctl0(), CTL0_I2CEN);
        set_bits(i2c.ctl0(), CTL0_ACKEN);
    }

    fn recover(&self) {
        let i2c = &self.peripheral;
        let mut spins = 0;
        while read_bits(i2c.stat1()) & STAT1_I2CBSY != 0 && spins < RELEASE_SPINS {
            spins += 1;
            spin_loop();
        }
        set_bits(i2c.ctl0(), CTL0_SRESET);
        self.pins.release();
        clear_bits(i2c.ctl0(), CTL0_SRESET);
        Channel::new().clear_flags();
        self.configure();
    }

    async fn write_all(&self, address: SevenBitAddress, chunks: &[&[u8]]) -> Result<(), Error> {
        let i2c = &self.peripheral;
        step(self.released()).await?;
        modify_bits(i2c.stat0(), |bits| bits & !error::STAT0_ERRORS);
        let mut transfer = Transfer::new(i2c);
        set_bits(i2c.ctl0(), CTL0_START);
        step(transfer.event(STAT0_SBSEND, NoAcknowledgeSource::Address)).await?;
        write_bits(i2c.data(), u32::from(address) << 1);
        step(transfer.event(STAT0_ADDSEND, NoAcknowledgeSource::Address)).await?;
        let _ = read_bits(i2c.stat0());
        let _ = read_bits(i2c.stat1());
        if chunks.iter().any(|chunk| !chunk.is_empty()) {
            critical_section::with(|section| modify_ctl1(section, i2c, |bits| bits | CTL1_DMAON));
            for chunk in chunks.iter().filter(|chunk| !chunk.is_empty()) {
                step(transfer.send(chunk)).await?;
            }
            step(transfer.event(STAT0_BTC, NoAcknowledgeSource::Data)).await?;
        }
        step(transfer.stop()).await
    }

    async fn released(&self) -> Result<(), Error> {
        while read_bits(self.peripheral.stat1()) & STAT1_I2CBSY != 0 {
            yield_now().await;
        }
        Ok(())
    }
}

#[must_use]
pub fn transfers() -> Bus {
    TRANSFERS.report()
}

pub(crate) fn stop_for_reset() {
    let i2c = registers();
    if read_bits(i2c.stat1()) & STAT1_MASTER == 0 {
        return;
    }
    Channel::new().clear_control(dma::ENABLE);
    set_bits(i2c.ctl0(), CTL0_STOP);
    let mut spins = 0;
    while read_bits(i2c.ctl0()) & CTL0_STOP != 0 && spins < RELEASE_SPINS {
        spins += 1;
        spin_loop();
    }
}

impl ErrorType for I2c {
    type Error = Error;
}

impl hal_i2c::I2c for I2c {
    async fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        if operations.is_empty() {
            return Ok(());
        }
        let chunks = match writes(operations) {
            Ok(chunks) => chunks,
            Err(error) => {
                let refused = Err(error);
                TRANSFERS.record(&refused);
                return refused;
            }
        };
        let result = self.write_all(address, &chunks).await;
        TRANSFERS.record(&result);
        if result.is_err() {
            self.recover();
        }
        result
    }
}

fn writes<'w>(operations: &'w [Operation<'_>]) -> Result<[&'w [u8]; MAX_WRITES], Error> {
    let mut chunks: [&[u8]; MAX_WRITES] = [&[]; MAX_WRITES];
    let mut count = 0;
    let mut total: usize = 0;
    for operation in operations {
        match operation {
            Operation::Read(_) => return Err(Error::Read),
            Operation::Write(bytes) => {
                total = total.saturating_add(bytes.len());
                if total > MAX_TRANSFER {
                    return Err(Error::TooLong(total));
                }
                let Some(slot) = chunks.get_mut(count) else {
                    return Err(Error::TooManyWrites);
                };
                *slot = bytes;
                count += 1;
            }
        }
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use openwoot::diagnostics::BusError;

    use super::{step::STEP_TIMEOUT, *};

    const BITS_PER_BYTE: u64 = 9;
    const ADDRESS_BYTES: u64 = 1;

    fn micros_on_the_bus(data_bytes: u64) -> u64 {
        (data_bytes + ADDRESS_BYTES) * BITS_PER_BYTE * 1_000_000 / u64::from(timing::BUS_HZ)
    }

    #[test]
    fn the_longest_accepted_transfer_is_the_longest_whose_data_fits_one_step_time_out() {
        let longest = u64::try_from(MAX_TRANSFER).unwrap();
        assert!(micros_on_the_bus(longest) <= STEP_TIMEOUT.as_micros());
        assert!(micros_on_the_bus(longest + 1) > STEP_TIMEOUT.as_micros());
    }

    #[test]
    fn writes_are_collected_in_order_and_reads_are_refused() {
        let first = [1_u8, 2];
        let second = [3_u8];
        let empty: &[u8] = &[];
        let mut target = [0_u8; 2];
        assert_eq!(
            writes(&[Operation::Write(&first), Operation::Write(&second)]),
            Ok([&first[..], &second[..], empty, empty])
        );
        assert_eq!(writes(&[Operation::Write(empty)]), Ok([empty; MAX_WRITES]));
        assert_eq!(
            writes(&[Operation::Write(&first), Operation::Read(&mut target)]),
            Err(Error::Read)
        );
    }

    #[test]
    fn a_transaction_that_cannot_end_within_the_time_out_is_refused() {
        let longest = [0_u8; MAX_TRANSFER];
        assert!(writes(&[Operation::Write(&longest)]).is_ok());
        assert_eq!(
            writes(&[Operation::Write(&longest), Operation::Write(&[0])]),
            Err(Error::TooLong(MAX_TRANSFER + 1))
        );
    }

    #[test]
    fn more_writes_than_the_transfer_can_chain_are_refused() {
        assert_eq!(
            writes(&[
                Operation::Write(&[1]),
                Operation::Write(&[2]),
                Operation::Write(&[3]),
                Operation::Write(&[4]),
                Operation::Write(&[5]),
            ]),
            Err(Error::TooManyWrites)
        );
    }

    #[test]
    fn every_refused_transaction_counts_as_an_other_bus_error() {
        for error in [
            Error::Read,
            Error::TooLong(MAX_TRANSFER + 1),
            Error::TooManyWrites,
        ] {
            assert_eq!(BusError::from(error), BusError::Other);
        }
    }
}
