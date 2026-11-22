use core::future::{Future, pending};

use embedded_storage_async::nor_flash::{ErrorType, MultiwriteNorFlash, NorFlash, ReadNorFlash};

#[derive(Debug)]
pub(crate) struct Stalled<F>(pub(crate) F);

impl<F: ErrorType> ErrorType for Stalled<F> {
    type Error = F::Error;
}

impl<F: ReadNorFlash> ReadNorFlash for Stalled<F> {
    const READ_SIZE: usize = F::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, bytes).await
    }

    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl<F: NorFlash> NorFlash for Stalled<F> {
    const ERASE_SIZE: usize = F::ERASE_SIZE;
    const WRITE_SIZE: usize = F::WRITE_SIZE;

    fn erase(&mut self, _from: u32, _to: u32) -> impl Future<Output = Result<(), Self::Error>> {
        pending()
    }

    fn write(
        &mut self,
        _offset: u32,
        _bytes: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>> {
        pending()
    }
}

impl<F: MultiwriteNorFlash> MultiwriteNorFlash for Stalled<F> {}
