use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::wire::{Decode, Encode};

use super::{
    partition::Partition,
    record::{Record, RecordKey, Scope},
    settings::{Error, Settings},
};

const CURRENT_FORMAT: Format = Format { generation: 1 };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    Kept,
    Formatted(FormatReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatReason {
    Requested,
    Unrecognised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
struct Format {
    generation: u16,
}

impl Record for Format {
    const ITEM: u8 = 0;
    const VERSION: u8 = 1;
}

impl<F: MultiwriteNorFlash> Settings<F> {
    pub fn open(
        &mut self,
        partition: Partition<F>,
        format_requested: bool,
    ) -> Result<Opened, Error> {
        self.attach(partition)?;
        if format_requested {
            self.format()?;
            return Ok(Opened::Formatted(FormatReason::Requested));
        }
        match self.stored_format() {
            Ok(Some(format)) if format == CURRENT_FORMAT => Ok(Opened::Kept),
            Ok(_)
            | Err(Error::Corrupted | Error::TooBig | Error::Unencodable | Error::Internal) => {
                self.format()?;
                Ok(Opened::Formatted(FormatReason::Unrecognised))
            }
            Err(error) => Err(error),
        }
    }

    fn stored_format(&mut self) -> Result<Option<Format>, Error> {
        let key = RecordKey::of::<Format>(Scope::Global);
        let mut found = None;
        let indexed = self.index(&mut |stored_key, stored| {
            if stored_key == key {
                found = stored.decode::<Format>();
            }
        });
        match indexed {
            Ok(()) => return Ok(found),
            Err(Error::Flash(kind)) => return Err(Error::Flash(kind)),
            Err(_) => {}
        }
        let repaired = self.load::<Format>(Scope::Global)?;
        let _ = self.index(&mut |_, _| {});
        Ok(repaired)
    }

    pub fn factory_reset(&mut self) -> Result<(), Error> {
        self.format()
    }

    fn format(&mut self) -> Result<(), Error> {
        let removed = self.remove(RecordKey::of::<Format>(Scope::Global));
        if let Err(Error::Flash(kind)) = removed {
            return Err(Error::Flash(kind));
        }
        self.erase()?;
        self.store(Scope::Global, &CURRENT_FORMAT)
    }
}
