use core::future::{Future, ready};

use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

pub(crate) const SECTOR: u32 = 4096;
const WORD: u32 = 4;
const ERASE_STEPS: u32 = 8;
const ERASE_STEP: usize = (SECTOR / ERASE_STEPS) as usize;
const ERASED: u8 = 0xFF;

#[derive(Clone, Debug)]
pub(crate) struct Nor {
    memory: Vec<u8>,
    erases: Vec<u32>,
    budget: Option<usize>,
    cut: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModelError {
    PowerCut,
    NotAligned,
    OutOfBounds,
}

impl Nor {
    pub(crate) fn new(sectors: u32) -> Self {
        Self {
            memory: vec![ERASED; usize::try_from(sectors * SECTOR).unwrap_or_default()],
            erases: vec![0; usize::try_from(sectors).unwrap_or_default()],
            budget: None,
            cut: false,
        }
    }

    pub(crate) fn cut_after(&self, steps: usize) -> Self {
        Self {
            budget: Some(steps),
            cut: false,
            ..self.clone()
        }
    }

    pub(crate) fn revived(&self) -> Self {
        Self {
            budget: None,
            cut: false,
            ..self.clone()
        }
    }

    pub(crate) fn was_cut(&self) -> bool {
        self.cut
    }

    pub(crate) fn erases(&self) -> &[u32] {
        &self.erases
    }

    pub(crate) fn sector(&self, index: u32) -> &[u8] {
        let start = usize::try_from(index * SECTOR).unwrap_or(usize::MAX);
        self.memory
            .get(start..)
            .and_then(|rest| rest.get(..SECTOR as usize))
            .unwrap_or_default()
    }

    pub(crate) fn sector_mut(&mut self, index: u32) -> &mut [u8] {
        let start = usize::try_from(index * SECTOR).unwrap_or(usize::MAX);
        self.memory
            .get_mut(start..)
            .and_then(|rest| rest.get_mut(..SECTOR as usize))
            .unwrap_or_default()
    }

    fn step(&mut self) -> bool {
        match &mut self.budget {
            Some(0) => {
                self.cut = true;
                true
            }
            Some(budget) => {
                *budget -= 1;
                false
            }
            None => false,
        }
    }

    fn range(&self, offset: u32, len: usize, alignment: u32) -> Result<(usize, usize), ModelError> {
        if self.cut {
            return Err(ModelError::PowerCut);
        }
        let len = u32::try_from(len).map_err(|_| ModelError::OutOfBounds)?;
        if !offset.is_multiple_of(alignment) || !len.is_multiple_of(alignment) {
            return Err(ModelError::NotAligned);
        }
        let start = usize::try_from(offset).map_err(|_| ModelError::OutOfBounds)?;
        let end = usize::try_from(offset + len).map_err(|_| ModelError::OutOfBounds)?;
        if end > self.memory.len() {
            return Err(ModelError::OutOfBounds);
        }
        Ok((start, end))
    }

    fn program(&mut self, start: usize, bytes: &[u8]) {
        for (cell, value) in self.memory.iter_mut().skip(start).zip(bytes) {
            *cell &= value;
        }
    }

    fn read_now(&self, offset: u32, bytes: &mut [u8]) -> Result<(), ModelError> {
        let (start, end) = self.range(offset, bytes.len(), 1)?;
        bytes.copy_from_slice(self.memory.get(start..end).unwrap_or_default());
        Ok(())
    }

    fn erase_now(&mut self, from: u32, to: u32) -> Result<(), ModelError> {
        let len = usize::try_from(to.saturating_sub(from)).unwrap_or(usize::MAX);
        self.range(from, len, SECTOR)?;
        for index in from / SECTOR..to / SECTOR {
            for step in 0..ERASE_STEPS as usize {
                if self.step() {
                    return Err(ModelError::PowerCut);
                }
                for cell in self
                    .sector_mut(index)
                    .iter_mut()
                    .skip(step * ERASE_STEP)
                    .take(ERASE_STEP)
                {
                    *cell = ERASED;
                }
            }
            if let Some(count) = self
                .erases
                .get_mut(usize::try_from(index).unwrap_or(usize::MAX))
            {
                *count += 1;
            }
        }
        Ok(())
    }

    fn write_now(&mut self, offset: u32, bytes: &[u8]) -> Result<(), ModelError> {
        let (start, _) = self.range(offset, bytes.len(), WORD)?;
        for (index, word) in bytes.chunks(WORD as usize).enumerate() {
            let cell = start + index * WORD as usize;
            if self.step() {
                self.program(cell, word.get(..word.len() / 2).unwrap_or_default());
                return Err(ModelError::PowerCut);
            }
            self.program(cell, word);
        }
        Ok(())
    }
}

impl NorFlashError for ModelError {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::PowerCut => NorFlashErrorKind::Other,
            Self::NotAligned => NorFlashErrorKind::NotAligned,
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
        }
    }
}

impl ErrorType for Nor {
    type Error = ModelError;
}

impl ReadNorFlash for Nor {
    const READ_SIZE: usize = 1;

    fn read(
        &mut self,
        offset: u32,
        bytes: &mut [u8],
    ) -> impl Future<Output = Result<(), ModelError>> {
        ready(self.read_now(offset, bytes))
    }

    fn capacity(&self) -> usize {
        self.memory.len()
    }
}

impl NorFlash for Nor {
    const ERASE_SIZE: usize = SECTOR as usize;
    const WRITE_SIZE: usize = WORD as usize;

    fn erase(&mut self, from: u32, to: u32) -> impl Future<Output = Result<(), ModelError>> {
        ready(self.erase_now(from, to))
    }

    fn write(&mut self, offset: u32, bytes: &[u8]) -> impl Future<Output = Result<(), ModelError>> {
        ready(self.write_now(offset, bytes))
    }
}

impl MultiwriteNorFlash for Nor {}
