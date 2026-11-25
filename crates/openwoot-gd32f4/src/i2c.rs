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
const STAGING_LEN: usize = 16;
const SRAM_BASE: usize = 0x2000_0000;

static TRANSFERS: Transfers = Transfers::new();

#[derive(Debug)]
pub struct I2c {
    peripheral: pac::I2c0,
    timing: Timing,
    pins: Pins,
    staging: [u8; STAGING_LEN],
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
            staging: [0; STAGING_LEN],
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

    async fn write_all(&mut self, address: SevenBitAddress, chunks: &[&[u8]]) -> Result<(), Error> {
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
                if lies_in_sram(chunk.as_ptr().addr()) {
                    step(transfer.send(chunk)).await?;
                    continue;
                }
                let mut rest = *chunk;
                while !rest.is_empty() {
                    let staged = stage(&mut self.staging, &mut rest);
                    step(transfer.send(staged)).await?;
                }
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

const fn lies_in_sram(address: usize) -> bool {
    address >= SRAM_BASE
}

fn stage<'a>(staging: &'a mut [u8; STAGING_LEN], rest: &mut &[u8]) -> &'a [u8] {
    let whole = *rest;
    let (piece, later) = whole.split_at_checked(STAGING_LEN).unwrap_or((whole, &[]));
    *rest = later;
    for (staged, byte) in staging.iter_mut().zip(piece) {
        *staged = *byte;
    }
    staging.get(..piece.len()).unwrap_or_default()
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

    #[test]
    fn only_addresses_from_the_sram_up_are_read_by_the_dma_without_staging() {
        for address in [
            0x0000_0000,
            0x0800_0000,
            0x0808_5D58,
            0x1000_0000,
            0x1FFF_FFFF,
        ] {
            assert!(!lies_in_sram(address), "{address:#010x}");
        }
        for address in [0x2000_0000, 0x2001_081F, 0x2002_FFFF] {
            assert!(lies_in_sram(address), "{address:#010x}");
        }
    }

    #[test]
    fn bytes_outside_the_sram_are_staged_in_order_a_buffer_at_a_time() {
        let mut source = [0_u8; 2 * STAGING_LEN + 5];
        for (byte, value) in source.iter_mut().zip(1_u8..) {
            *byte = value;
        }
        let mut staging = [0; STAGING_LEN];
        let mut rest = &source[..];
        let mut sent = [0_u8; 2 * STAGING_LEN + 5];
        let mut lengths = [0_usize; 3];
        let mut written = 0;
        for length in &mut lengths {
            let staged = stage(&mut staging, &mut rest);
            *length = staged.len();
            sent[written..written + staged.len()].copy_from_slice(staged);
            written += staged.len();
        }
        assert_eq!(lengths, [STAGING_LEN, STAGING_LEN, 5]);
        assert_eq!(sent, source);
        assert_eq!(rest, b"");
        assert_eq!(stage(&mut staging, &mut rest), b"");
    }
}
