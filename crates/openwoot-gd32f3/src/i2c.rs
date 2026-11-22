mod error;
mod pins;
mod registers;
mod timing;
mod transfer;

use embassy_futures::yield_now;
use embassy_time::{Duration, with_timeout};
use embedded_hal_async::i2c::{
    self as hal_i2c, ErrorType, NoAcknowledgeSource, Operation, SevenBitAddress,
};
pub use error::Error;
use openwoot::diagnostics::{Bus, Transfers};
pub use pins::{ClockPin, DataPin};
use registers::{
    CTL0_ACKEN, CTL0_I2CEN, CTL0_SMBEN, CTL0_SRESET, CTL0_START, CTL1_DMAON, CTL1_I2CCLK,
    STAT0_ADDSEND, STAT0_BTC, STAT0_SBSEND, STAT1_I2CBSY,
};
use timing::{Timing, timing};
pub use transfer::InterruptHandler;
use transfer::{Channel, Transfer};

use crate::{
    interrupt::{self, Binding, Priority, typelevel},
    pac,
    rcu::{self, Clocks},
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

const TRANSFER_TIMEOUT: Duration = Duration::from_millis(10);
const APB1EN_I2C0EN: u32 = 1 << 21;
const OWN_ADDRESS: u32 = 0xFF;
const MAX_TRANSFER: usize = 0xFFFF;

static TRANSFERS: Transfers = Transfers::new();

#[derive(Debug)]
pub struct I2c {
    i2c: pac::I2c0,
    timing: Timing,
}

impl I2c {
    #[must_use]
    pub fn new(
        i2c: pac::I2c0,
        clock: impl ClockPin,
        data: impl DataPin,
        _irqs: impl Binding<typelevel::I2C0_EV, InterruptHandler>
        + Binding<typelevel::I2C0_ER, InterruptHandler>
        + Binding<typelevel::DMA0_Channel5, InterruptHandler>,
        clocks: &Clocks,
    ) -> Self {
        rcu::with_registers(|rcu| set_bits(rcu.apb1en(), APB1EN_I2C0EN));
        Channel::new().enable_clock();
        clock.into_i2c();
        data.into_i2c();
        let bus = Self {
            i2c,
            timing: timing(clocks.apb1),
        };
        bus.configure();
        for irq in [
            pac::Interrupt::I2C0_EV,
            pac::Interrupt::I2C0_ER,
            pac::Interrupt::DMA0_Channel5,
        ] {
            interrupt::enable(irq, Priority::HIGHEST);
        }
        bus
    }

    fn configure(&self) {
        let i2c = &self.i2c;
        modify_bits(i2c.ctl1(), |bits| {
            (bits & !CTL1_I2CCLK) | self.timing.clock_mhz
        });
        write_bits(i2c.rt(), self.timing.rise_time);
        write_bits(i2c.ckcfg(), self.timing.clock_configuration);
        write_bits(i2c.saddr0(), OWN_ADDRESS);
        clear_bits(i2c.ctl0(), CTL0_SMBEN);
        set_bits(i2c.ctl0(), CTL0_I2CEN);
        set_bits(i2c.ctl0(), CTL0_ACKEN);
    }

    fn recover(&self) {
        let i2c = &self.i2c;
        set_bits(i2c.ctl0(), CTL0_SRESET);
        clear_bits(i2c.ctl0(), CTL0_SRESET);
        Channel::new().clear_flags();
        self.configure();
    }

    async fn write_all(&self, address: SevenBitAddress, chunks: &[&[u8]]) -> Result<(), Error> {
        let i2c = &self.i2c;
        while read_bits(i2c.stat1()) & STAT1_I2CBSY != 0 {
            yield_now().await;
        }
        modify_bits(i2c.stat0(), |bits| bits & !error::STAT0_ERRORS);
        let mut transfer = Transfer::new(i2c);
        set_bits(i2c.ctl0(), CTL0_START);
        transfer
            .event(STAT0_SBSEND, NoAcknowledgeSource::Address)
            .await?;
        write_bits(i2c.data(), u32::from(address) << 1);
        transfer
            .event(STAT0_ADDSEND, NoAcknowledgeSource::Address)
            .await?;
        let _ = read_bits(i2c.stat0());
        let _ = read_bits(i2c.stat1());
        set_bits(i2c.ctl1(), CTL1_DMAON);
        for chunk in chunks.iter().filter(|chunk| !chunk.is_empty()) {
            transfer.send(chunk).await?;
        }
        transfer.event(STAT0_BTC, NoAcknowledgeSource::Data).await?;
        transfer.stop().await;
        Ok(())
    }
}

#[must_use]
pub fn transfers() -> Bus {
    TRANSFERS.report()
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
        let mut chunks: [&[u8]; 4] = [&[]; 4];
        let mut count = 0;
        for operation in operations.iter() {
            match operation {
                Operation::Read(_) => return Err(Error::Read),
                Operation::Write(bytes) => {
                    if bytes.len() > MAX_TRANSFER {
                        return Err(Error::TooLong(bytes.len()));
                    }
                    let Some(slot) = chunks.get_mut(count) else {
                        return Err(Error::TooManyWrites);
                    };
                    *slot = bytes;
                    count += 1;
                }
            }
        }
        let result = match with_timeout(TRANSFER_TIMEOUT, self.write_all(address, &chunks)).await {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout),
        };
        TRANSFERS.record(&result);
        if result.is_err() {
            self.recover();
        }
        result
    }
}
