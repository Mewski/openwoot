mod device;
mod pins;
mod registers;
mod transfer;

pub use device::Device;
use embedded_hal::spi::{ErrorKind, ErrorType};
use embedded_hal_async::spi::SpiBus;
use openwoot::diagnostics::{Bus, BusError, Transfers};
pub use pins::{ClockPin, DataInPin, DataOutPin};
use registers::{APB2EN_SPI0EN, APB2RST_SPI0RST, CTL0_SPIEN, I2SCTL_I2SSEL, control};
pub use transfer::InterruptHandler;
use transfer::{ReceiveChannel, Sink, Source, Transfer, wait_until_shifted_out};

use crate::{
    interrupt::{self, Binding, Priority, typelevel},
    pac, rcu,
    register::{clear_bits, modify_bits, set_bits},
};

const MAX_TRANSFER: usize = 0xFFFF;
const STAGING_LEN: usize = 16;
const SRAM_BASE: usize = 0x2000_0000;

static TRANSFERS: Transfers = Transfers::new();

#[derive(Debug)]
pub struct Spi {
    spi: pac::Spi0,
    staging: [u8; STAGING_LEN],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the SPI DMA channel reported a transfer error")]
    Transfer,
}

impl Spi {
    #[must_use]
    pub fn new(
        spi: pac::Spi0,
        clock: impl ClockPin,
        data_out: impl DataOutPin,
        data_in: impl DataInPin,
        _irqs: impl Binding<typelevel::DMA1_Channel2, InterruptHandler>,
    ) -> Self {
        rcu::with_registers(|rcu| {
            set_bits(rcu.apb2en(), APB2EN_SPI0EN);
            set_bits(rcu.apb2rst(), APB2RST_SPI0RST);
            clear_bits(rcu.apb2rst(), APB2RST_SPI0RST);
        });
        ReceiveChannel::new().enable_clock();
        clock.into_spi();
        data_out.into_spi();
        data_in.into_spi();
        modify_bits(spi.ctl0(), control);
        clear_bits(spi.i2sctl(), I2SCTL_I2SSEL);
        set_bits(spi.ctl0(), CTL0_SPIEN);
        interrupt::enable(pac::Interrupt::DMA1_Channel2, Priority::HIGHEST);
        Self {
            spi,
            staging: [0; STAGING_LEN],
        }
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

    async fn exchange_words(&mut self, words: &[u8], sink: Sink) -> Result<(), Error> {
        if lies_in_sram(words.as_ptr().addr()) {
            return self
                .exchange(Source::Bytes(words.as_ptr()), sink, words.len())
                .await;
        }
        let mut done = 0;
        for piece in words.chunks(STAGING_LEN) {
            for (staged, byte) in self.staging.iter_mut().zip(piece) {
                *staged = *byte;
            }
            self.exchange(
                Source::Bytes(self.staging.as_ptr()),
                sink.advanced(done),
                piece.len(),
            )
            .await?;
            done += piece.len();
        }
        Ok(())
    }

    fn idle(&self) {
        wait_until_shifted_out(&self.spi);
    }
}

#[must_use]
pub fn transfers() -> Bus {
    TRANSFERS.report()
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl From<Error> for BusError {
    fn from(error: Error) -> Self {
        match error {
            Error::Transfer => Self::Dma,
        }
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
        self.exchange_words(words, Sink::Discard).await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        let common = read.len().min(write.len());
        let (read_common, read_rest) = read.split_at_mut(common);
        let (write_common, write_rest) = write.split_at(common);
        self.exchange_words(write_common, Sink::Bytes(read_common.as_mut_ptr()))
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

const fn lies_in_sram(address: usize) -> bool {
    address >= SRAM_BASE
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
