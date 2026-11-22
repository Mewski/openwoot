use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

use super::nor_layout::SettingsRegion;

#[derive(Debug)]
pub struct Partition<F> {
    flash: F,
    start: u32,
    len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PartitionError<E> {
    #[error("could not access the flash: {0:?}")]
    Flash(E),
    #[error("the access lies outside the settings partition")]
    OutOfBounds,
}

impl<F: NorFlash> Partition<F> {
    #[must_use]
    pub const fn new(flash: F, region: SettingsRegion) -> Self {
        let region = region.region();
        Self {
            flash,
            start: region.start,
            len: region.end - region.start,
        }
    }

    #[must_use]
    pub const fn len(&self) -> u32 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn release(self) -> F {
        self.flash
    }

    fn absolute<E>(&self, offset: u32, len: usize) -> Result<u32, PartitionError<E>> {
        let len = u32::try_from(len).map_err(|_| PartitionError::OutOfBounds)?;
        match offset.checked_add(len) {
            Some(end) if end <= self.len => Ok(self.start + offset),
            _ => Err(PartitionError::OutOfBounds),
        }
    }
}

impl<E: NorFlashError> NorFlashError for PartitionError<E> {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::Flash(error) => error.kind(),
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
        }
    }
}

impl<F: NorFlash> ErrorType for Partition<F> {
    type Error = PartitionError<F::Error>;
}

impl<F: NorFlash> ReadNorFlash for Partition<F> {
    const READ_SIZE: usize = F::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let address = self.absolute(offset, bytes.len())?;
        self.flash
            .read(address, bytes)
            .await
            .map_err(PartitionError::Flash)
    }

    fn capacity(&self) -> usize {
        usize::try_from(self.len).unwrap_or_default()
    }
}

impl<F: NorFlash> NorFlash for Partition<F> {
    const ERASE_SIZE: usize = F::ERASE_SIZE;
    const WRITE_SIZE: usize = F::WRITE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let len = to.checked_sub(from).ok_or(PartitionError::OutOfBounds)?;
        let start = self.absolute(from, usize::try_from(len).unwrap_or(usize::MAX))?;
        self.flash
            .erase(start, start + len)
            .await
            .map_err(PartitionError::Flash)
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let address = self.absolute(offset, bytes.len())?;
        self.flash
            .write(address, bytes)
            .await
            .map_err(PartitionError::Flash)
    }
}

impl<F: MultiwriteNorFlash> MultiwriteNorFlash for Partition<F> {}
