use embassy_futures::yield_now;
use embedded_hal_async::spi::{Operation, SpiDevice};
use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

const PAGE_PROGRAM: u8 = 0x02;
const READ_STATUS: u8 = 0x05;
const WRITE_ENABLE: u8 = 0x06;
const FAST_READ: u8 = 0x0B;
const SECTOR_ERASE: u8 = 0x20;
const UNIQUE_ID: u8 = 0x4B;
const READ_ID: u8 = 0x90;
const STATUS_BUSY: u8 = 1 << 0;
const STATUS_WRITE_ENABLED: u8 = 1 << 1;
const DUMMY: u8 = 0xFF;
const UNIQUE_ID_DUMMY_BYTES: usize = 4;
const ADDRESSABLE: u32 = 1 << 24;
const READY_POLLS: u32 = 100_000;
const PACED_READY_POLLS: u32 = 20_000;
const READY_POLL_PAUSE_NANOSECONDS: u32 = 100_000;

pub const PAGE_SIZE: u32 = 256;
pub const SECTOR_SIZE: u32 = 4096;
pub const WINBOND: u8 = 0xEF;
pub const GIGADEVICE: u8 = 0xC8;
pub const DEVICE_64_MBIT: u8 = 0x16;
pub const UNIQUE_ID_LEN: usize = 8;

#[derive(Debug)]
pub struct SpiNor<D> {
    device: D,
    capacity: u32,
    stalled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identification {
    pub manufacturer: u8,
    pub device: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error<E> {
    #[error("could not transfer over the SPI bus: {0:?}")]
    Bus(E),
    #[error("the flash access is not aligned to the program or erase size")]
    NotAligned,
    #[error("the flash access lies outside the chip")]
    OutOfBounds,
    #[error("the flash chip did not become ready")]
    NotReady,
    #[error("the flash chip did not accept the write enable")]
    WriteRefused,
}

impl Identification {
    #[must_use]
    pub const fn is_known(self) -> bool {
        matches!(self.manufacturer, WINBOND | GIGADEVICE) && self.device == DEVICE_64_MBIT
    }
}

impl<D: SpiDevice> SpiNor<D> {
    #[must_use]
    pub const fn new(device: D, capacity: u32) -> Self {
        Self {
            device,
            capacity,
            stalled: false,
        }
    }

    pub fn release(self) -> D {
        self.device
    }

    pub async fn identification(&mut self) -> Result<Identification, Error<D::Error>> {
        self.wait_ready().await?;
        let mut id = [0; 2];
        self.device
            .transaction(&mut [
                Operation::Write(&command(READ_ID, 0)),
                Operation::Read(&mut id),
            ])
            .await
            .map_err(Error::Bus)?;
        let [manufacturer, device] = id;
        Ok(Identification {
            manufacturer,
            device,
        })
    }

    pub async fn unique_id(&mut self) -> Result<[u8; UNIQUE_ID_LEN], Error<D::Error>> {
        self.wait_ready().await?;
        let mut id = [0; UNIQUE_ID_LEN];
        self.device
            .transaction(&mut [
                Operation::Write(&command(UNIQUE_ID, 0)),
                Operation::Write(&[DUMMY; UNIQUE_ID_DUMMY_BYTES]),
                Operation::Read(&mut id),
            ])
            .await
            .map_err(Error::Bus)?;
        Ok(id)
    }

    pub async fn wait_ready(&mut self) -> Result<(), Error<D::Error>> {
        let polls = if self.stalled {
            1
        } else {
            READY_POLLS + PACED_READY_POLLS
        };
        for poll in 0..polls {
            if self.status().await? & STATUS_BUSY == 0 {
                self.stalled = false;
                return Ok(());
            }
            if poll >= READY_POLLS {
                self.pause().await?;
            }
            yield_now().await;
        }
        self.stalled = true;
        Err(Error::NotReady)
    }

    async fn status(&mut self) -> Result<u8, Error<D::Error>> {
        let mut status = [0];
        self.device
            .transaction(&mut [
                Operation::Write(&[READ_STATUS]),
                Operation::Read(&mut status),
            ])
            .await
            .map_err(Error::Bus)?;
        let [status] = status;
        Ok(status)
    }

    async fn pause(&mut self) -> Result<(), Error<D::Error>> {
        self.device
            .transaction(&mut [Operation::DelayNs(READY_POLL_PAUSE_NANOSECONDS)])
            .await
            .map_err(Error::Bus)
    }

    async fn enable_write(&mut self) -> Result<(), Error<D::Error>> {
        self.device
            .write(&[WRITE_ENABLE])
            .await
            .map_err(Error::Bus)?;
        if self.status().await? & STATUS_WRITE_ENABLED == 0 {
            return Err(Error::WriteRefused);
        }
        Ok(())
    }

    async fn program_page(&mut self, address: u32, bytes: &[u8]) -> Result<(), Error<D::Error>> {
        self.wait_ready().await?;
        self.enable_write().await?;
        self.device
            .transaction(&mut [
                Operation::Write(&command(PAGE_PROGRAM, address)),
                Operation::Write(bytes),
            ])
            .await
            .map_err(Error::Bus)?;
        self.wait_ready().await
    }

    async fn erase_sector(&mut self, address: u32) -> Result<(), Error<D::Error>> {
        self.wait_ready().await?;
        self.enable_write().await?;
        self.device
            .write(&command(SECTOR_ERASE, address))
            .await
            .map_err(Error::Bus)?;
        self.wait_ready().await
    }
}

impl<E: core::fmt::Debug> NorFlashError for Error<E> {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::Bus(_) | Self::NotReady | Self::WriteRefused => NorFlashErrorKind::Other,
            Self::NotAligned => NorFlashErrorKind::NotAligned,
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
        }
    }
}

impl<D: SpiDevice> ErrorType for SpiNor<D> {
    type Error = Error<D::Error>;
}

impl<D: SpiDevice> ReadNorFlash for SpiNor<D> {
    const READ_SIZE: usize = 1;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.check(offset, bytes.len(), 1)?;
        if bytes.is_empty() {
            return Ok(());
        }
        self.wait_ready().await?;
        self.device
            .transaction(&mut [
                Operation::Write(&command(FAST_READ, offset)),
                Operation::Write(&[DUMMY]),
                Operation::Read(bytes),
            ])
            .await
            .map_err(Error::Bus)
    }

    fn capacity(&self) -> usize {
        usize::try_from(self.capacity).unwrap_or(usize::MAX)
    }
}

impl<D: SpiDevice> NorFlash for SpiNor<D> {
    const ERASE_SIZE: usize = SECTOR_SIZE as usize;
    const WRITE_SIZE: usize = 4;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        if to < from {
            return Err(Error::OutOfBounds);
        }
        self.check(from, (to - from) as usize, SECTOR_SIZE)?;
        let mut address = from;
        while address < to {
            self.erase_sector(address).await?;
            address += SECTOR_SIZE;
        }
        Ok(())
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.check(offset, bytes.len(), 4)?;
        let mut address = offset;
        let mut rest = bytes;
        while !rest.is_empty() {
            let room = PAGE_SIZE - address % PAGE_SIZE;
            let (page, next) = rest.split_at(rest.len().min(room as usize));
            self.program_page(address, page).await?;
            address += room;
            rest = next;
        }
        Ok(())
    }
}

impl<D: SpiDevice> MultiwriteNorFlash for SpiNor<D> {}

const fn command(opcode: u8, address: u32) -> [u8; 4] {
    let [_, high, middle, low] = address.to_be_bytes();
    [opcode, high, middle, low]
}

impl<D> SpiNor<D> {
    fn check<E>(&self, offset: u32, len: usize, alignment: u32) -> Result<(), Error<E>> {
        let len = u32::try_from(len).map_err(|_| Error::OutOfBounds)?;
        if !offset.is_multiple_of(alignment) || !len.is_multiple_of(alignment) {
            return Err(Error::NotAligned);
        }
        match offset.checked_add(len) {
            Some(end) if end <= self.capacity && end <= ADDRESSABLE => Ok(()),
            _ => Err(Error::OutOfBounds),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_carry_the_address_most_significant_byte_first() {
        assert_eq!(command(FAST_READ, 0x7F_F123), [0x0B, 0x7F, 0xF1, 0x23]);
        assert_eq!(command(READ_ID, 0), [0x90, 0, 0, 0]);
    }

    #[test]
    fn only_64_megabit_parts_of_both_makers_are_known() {
        for (manufacturer, device, known) in [
            (WINBOND, DEVICE_64_MBIT, true),
            (GIGADEVICE, DEVICE_64_MBIT, true),
            (WINBOND, 0x15, false),
            (0x20, DEVICE_64_MBIT, false),
        ] {
            let id = Identification {
                manufacturer,
                device,
            };
            assert_eq!(id.is_known(), known);
        }
    }
}
