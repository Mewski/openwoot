use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use serde::{Deserialize, Serialize};

use super::{
    partition::Partition,
    record::{Record, Scope},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Format {
    generation: u16,
}

impl Record for Format {
    const ITEM: u8 = 0;
    const VERSION: u8 = 1;
}

impl<F: MultiwriteNorFlash> Settings<F> {
    pub async fn open(
        &mut self,
        partition: Partition<F>,
        format_requested: bool,
    ) -> Result<Opened, Error> {
        self.attach(partition)?;
        if format_requested {
            self.format().await?;
            return Ok(Opened::Formatted(FormatReason::Requested));
        }
        match self.load::<Format>(Scope::Global).await {
            Ok(Some(format)) if format == CURRENT_FORMAT => Ok(Opened::Kept),
            Ok(_) | Err(Error::Corrupted | Error::Unencodable | Error::Internal) => {
                self.format().await?;
                Ok(Opened::Formatted(FormatReason::Unrecognised))
            }
            Err(error) => Err(error),
        }
    }

    pub async fn factory_reset(&mut self) -> Result<(), Error> {
        self.format().await
    }

    async fn format(&mut self) -> Result<(), Error> {
        self.erase().await?;
        self.store(Scope::Global, &CURRENT_FORMAT).await
    }
}
