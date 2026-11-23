use std::{cell::Cell, rc::Rc};

use embedded_storage_async::nor_flash::{ErrorType, MultiwriteNorFlash, NorFlash, ReadNorFlash};

use super::model::{ModelError, Nor};

#[derive(Debug)]
pub(crate) struct Probe {
    nor: Nor,
    watch: Rc<Watch>,
}

#[derive(Debug, Default)]
pub(crate) struct Watch {
    reads: Cell<u32>,
    failing: Cell<bool>,
}

impl Probe {
    pub(crate) fn new(nor: Nor) -> (Self, Rc<Watch>) {
        let watch = Rc::new(Watch::default());
        (
            Self {
                nor,
                watch: Rc::clone(&watch),
            },
            watch,
        )
    }

    fn writable(&self) -> Result<(), ModelError> {
        if self.watch.failing.get() {
            Err(ModelError::PowerCut)
        } else {
            Ok(())
        }
    }
}

impl Watch {
    pub(crate) fn reads(&self) -> u32 {
        self.reads.get()
    }

    pub(crate) fn fail_writes(&self, failing: bool) {
        self.failing.set(failing);
    }
}

impl ErrorType for Probe {
    type Error = ModelError;
}

impl ReadNorFlash for Probe {
    const READ_SIZE: usize = <Nor as ReadNorFlash>::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), ModelError> {
        self.watch.reads.set(self.watch.reads.get() + 1);
        self.nor.read(offset, bytes).await
    }

    fn capacity(&self) -> usize {
        self.nor.capacity()
    }
}

impl NorFlash for Probe {
    const ERASE_SIZE: usize = <Nor as NorFlash>::ERASE_SIZE;
    const WRITE_SIZE: usize = <Nor as NorFlash>::WRITE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), ModelError> {
        self.writable()?;
        self.nor.erase(from, to).await
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ModelError> {
        self.writable()?;
        self.nor.write(offset, bytes).await
    }
}

impl MultiwriteNorFlash for Probe {}
