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
    const VERSION: u16 = 1;
}

impl<F: MultiwriteNorFlash> Settings<F> {
    pub async fn open(
        partition: Partition<F>,
        format_requested: bool,
    ) -> Result<(Self, Opened), Error> {
        let mut settings = Self::new(partition)?;
        if format_requested {
            settings.format().await?;
            return Ok((settings, Opened::Formatted(FormatReason::Requested)));
        }
        match settings.load::<Format>(Scope::Global).await {
            Ok(Some(format)) if format == CURRENT_FORMAT => Ok((settings, Opened::Kept)),
            Ok(_) | Err(Error::Corrupted | Error::Encoding | Error::Internal) => {
                settings.format().await?;
                Ok((settings, Opened::Formatted(FormatReason::Unrecognised)))
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
