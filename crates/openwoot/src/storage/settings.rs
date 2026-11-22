use core::fmt;

use embedded_storage_async::nor_flash::{
    MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind,
};
use sequential_storage::{
    self as store,
    cache::{
        Cache, key_pointers::ArrayKeyPointers, page_pointers::ArrayPagePointers,
        page_states::ArrayPageStates,
    },
    map::{MapConfig, MapStorage},
};
use serde::{Deserialize, Serialize};

use super::{
    Partition, PartitionError,
    record::{Encoding, Key, Record, Scope, Stored},
};

pub const MAX_PAGES: usize = 128;
pub const MAX_KEYS: usize = 256;
pub const MAX_VALUE_LEN: usize = 2048;

const KEY_LEN: usize = 2;
const VERSION_LEN: usize = 2;
const BUFFER_LEN: usize = KEY_LEN + VERSION_LEN + MAX_VALUE_LEN;
const CURRENT_FORMAT: Format = Format { generation: 1 };

type SettingsCache = Cache<
    ArrayPageStates<MAX_PAGES>,
    ArrayPagePointers<MAX_PAGES>,
    ArrayKeyPointers<Key, MAX_KEYS>,
    Key,
>;

type Map<F> = MapStorage<Key, Partition<F>, SettingsCache>;

pub struct Settings<F: MultiwriteNorFlash> {
    map: Option<Map<F>>,
    len: u32,
    buffer: Buffer,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("could not access the settings flash: {0:?}")]
    Flash(NorFlashErrorKind),
    #[error("the settings flash is full")]
    Full,
    #[error("the settings flash is corrupted")]
    Corrupted,
    #[error("a setting is larger than {MAX_VALUE_LEN} bytes")]
    TooBig,
    #[error("could not encode a setting")]
    Encoding,
    #[error("the settings partition holds fewer than two or more than {MAX_PAGES} sectors")]
    Layout,
    #[error("the settings store reported an internal error")]
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Format {
    generation: u16,
}

#[repr(align(4))]
struct Buffer([u8; BUFFER_LEN]);

impl Record for Format {
    const ITEM: u8 = 0;
    const VERSION: u16 = 1;
}

impl<F: MultiwriteNorFlash> Settings<F> {
    pub async fn open(
        partition: Partition<F>,
        format_requested: bool,
    ) -> Result<(Self, Opened), Error> {
        let len = partition.len();
        let pages = usize::try_from(len).unwrap_or(usize::MAX) / F::ERASE_SIZE;
        if pages > MAX_PAGES {
            return Err(Error::Layout);
        }
        let mut settings = Self {
            map: Some(MapStorage::new(partition, config(len)?, cache())),
            len,
            buffer: Buffer([0; BUFFER_LEN]),
        };
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

    pub async fn load<R: Record>(&mut self, scope: Scope) -> Result<Option<R>, Error> {
        let Self { map, buffer, .. } = self;
        let stored = map
            .as_mut()
            .ok_or(Error::Internal)?
            .fetch_item::<Stored<'_>>(&mut buffer.0, &Key::of::<R>(scope))
            .await
            .map_err(Error::from_store)?;
        Ok(stored.and_then(|stored| stored.decode()))
    }

    pub async fn store<R: Record>(&mut self, scope: Scope, record: &R) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .store_item(&mut buffer.0, &Key::of::<R>(scope), &Encoding(record))
            .await
            .map_err(Error::from_store)
    }

    pub async fn remove(&mut self, key: Key) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .remove_item(&mut buffer.0, &key)
            .await
            .map_err(Error::from_store)
    }

    pub async fn visit(&mut self, mut visit: impl FnMut(Key, Stored<'_>)) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        let mut items = map
            .as_mut()
            .ok_or(Error::Internal)?
            .fetch_all_items(&mut buffer.0)
            .await
            .map_err(Error::from_store)?;
        while let Some((key, stored)) = items
            .next::<Stored<'_>>(&mut buffer.0)
            .await
            .map_err(Error::from_store)?
        {
            visit(key, stored);
        }
        Ok(())
    }

    pub async fn factory_reset(&mut self) -> Result<(), Error> {
        self.format().await
    }

    pub fn release(self) -> Option<Partition<F>> {
        self.map.map(|map| map.destroy().0)
    }

    async fn format(&mut self) -> Result<(), Error> {
        let (mut partition, _) = self.map.take().ok_or(Error::Internal)?.destroy();
        let erased = partition.erase(0, self.len).await;
        self.map = Some(MapStorage::new(partition, config(self.len)?, cache()));
        erased.map_err(|error| Error::Flash(error.kind()))?;
        self.store(Scope::Global, &CURRENT_FORMAT).await
    }
}

impl<F: MultiwriteNorFlash> fmt::Debug for Settings<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("len", &self.len)
            .field("open", &self.map.is_some())
            .finish_non_exhaustive()
    }
}

impl Error {
    fn from_store<E: NorFlashError>(error: store::Error<PartitionError<E>>) -> Self {
        match error {
            store::Error::Storage { value, .. } => Self::Flash(value.kind()),
            store::Error::FullStorage => Self::Full,
            store::Error::Corrupted { .. } => Self::Corrupted,
            store::Error::ItemTooBig | store::Error::BufferTooSmall(_) => Self::TooBig,
            store::Error::SerializationError(_) => Self::Encoding,
            _ => Self::Internal,
        }
    }
}

fn cache() -> SettingsCache {
    Cache::new(
        ArrayPageStates::new(),
        ArrayPagePointers::new(),
        ArrayKeyPointers::new(),
    )
}

fn config<F: NorFlash>(len: u32) -> Result<MapConfig<F>, Error> {
    MapConfig::try_new(0..len).map_err(|_| Error::Layout)
}
