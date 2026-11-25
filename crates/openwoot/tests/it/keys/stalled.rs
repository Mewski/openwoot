use core::{
    future::poll_fn,
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};

use embedded_storage_async::nor_flash::{ErrorType, MultiwriteNorFlash, NorFlash, ReadNorFlash};

#[derive(Debug, Default)]
pub(crate) struct Gate {
    waiting: AtomicBool,
    open: AtomicBool,
}

impl Gate {
    pub(crate) fn is_waiting(&self) -> bool {
        self.waiting.load(Ordering::Acquire)
    }

    pub(crate) fn open(&self) {
        self.open.store(true, Ordering::Release);
    }

    async fn pass(&self) {
        poll_fn(|_| {
            self.waiting.store(true, Ordering::Release);
            if self.open.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                std::thread::yield_now();
                Poll::Pending
            }
        })
        .await;
    }
}

#[derive(Debug)]
pub(crate) struct Stalled<'a, F> {
    pub(crate) flash: F,
    pub(crate) gate: &'a Gate,
}

impl<F: ErrorType> ErrorType for Stalled<'_, F> {
    type Error = F::Error;
}

impl<F: ReadNorFlash> ReadNorFlash for Stalled<'_, F> {
    const READ_SIZE: usize = F::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.flash.read(offset, bytes).await
    }

    fn capacity(&self) -> usize {
        self.flash.capacity()
    }
}

impl<F: NorFlash> NorFlash for Stalled<'_, F> {
    const ERASE_SIZE: usize = F::ERASE_SIZE;
    const WRITE_SIZE: usize = F::WRITE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.gate.pass().await;
        self.flash.erase(from, to).await
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.gate.pass().await;
        self.flash.write(offset, bytes).await
    }
}

impl<F: MultiwriteNorFlash> MultiwriteNorFlash for Stalled<'_, F> {}
