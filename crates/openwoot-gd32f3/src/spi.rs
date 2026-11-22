mod device;
mod pins;
mod registers;
mod transfer;

use core::hint::spin_loop;

pub use device::Device;
use embedded_hal::spi::{ErrorKind, ErrorType};
use embedded_hal_async::spi::SpiBus;
pub use pins::{ClockPin, DataInPin, DataOutPin};
use registers::{CTL0_SPIEN, I2SCTL_I2SSEL, STAT_TRANS, control};
pub use transfer::InterruptHandler;
use transfer::{ReceiveChannel, Sink, Source, Transfer};

use crate::{
    interrupt::{self, Binding, Priority, typelevel},
    pac, rcu,
    register::{clear_bits, modify_bits, read_bits, set_bits},
};

const APB1EN_SPI1EN: u32 = 1 << 14;
const MAX_TRANSFER: usize = 0xFFFF;

#[derive(Debug)]
pub struct Spi {
    spi: pac::Spi1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the SPI DMA channel reported a transfer error")]
    Transfer,
}

impl Spi {
    #[must_use]
    pub fn new(
        spi: pac::Spi1,
        clock: impl ClockPin,
        data_out: impl DataOutPin,
        data_in: impl DataInPin,
        _irq: impl Binding<typelevel::DMA0_Channel3, InterruptHandler>,
    ) -> Self {
        rcu::with_registers(|rcu| set_bits(rcu.apb1en(), APB1EN_SPI1EN));
        ReceiveChannel::new().enable_clock();
        clock.into_spi();
        data_out.into_spi();
        data_in.into_spi();
        modify_bits(spi.ctl0(), control);
        clear_bits(spi.i2sctl(), I2SCTL_I2SSEL);
        set_bits(spi.ctl0(), CTL0_SPIEN);
        interrupt::enable(pac::Interrupt::DMA0_Channel3, Priority::HIGHEST);
        Self { spi }
    }

    async fn exchange(&mut self, source: Source, sink: Sink, len: usize) -> Result<(), Error> {
        let mut done = 0;
        while done < len {
            let chunk = (len - done).min(MAX_TRANSFER);
            let transfer =
                Transfer::start(&self.spi, source.advanced(done), sink.advanced(done), chunk);
            transfer.finished().await?;
            done += chunk;
        }
        Ok(())
    }

    fn idle(&self) {
        while read_bits(self.spi.stat()) & STAT_TRANS != 0 {
            spin_loop();
        }
    }
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl ErrorType for Spi {
    type Error = Error;
}

impl SpiBus for Spi {
    async fn read(&mut self, words: &mut [u8]) -> Result<(), Error> {
        self.exchange(Source::Idle, Sink::Bytes(words.as_mut_ptr()), words.len())
            .await
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Error> {
        self.exchange(Source::Bytes(words.as_ptr()), Sink::Discard, words.len())
            .await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        let common = read.len().min(write.len());
        let (read_common, read_rest) = read.split_at_mut(common);
        let (write_common, write_rest) = write.split_at(common);
        self.exchange(
            Source::Bytes(write_common.as_ptr()),
            Sink::Bytes(read_common.as_mut_ptr()),
            common,
        )
        .await?;
        self.read(read_rest).await?;
        self.write(write_rest).await
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Error> {
        let len = words.len();
        let pointer = words.as_mut_ptr();
        self.exchange(Source::Bytes(pointer), Sink::Bytes(pointer), len)
            .await
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> {
        self.idle();
        core::future::ready(Ok(()))
    }
}
