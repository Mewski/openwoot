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

use super::{
    partition::{Partition, PartitionError},
    record::{Encoder, Encoding, KEY_LEN, Record, RecordKey, Scope, Stored, VERSION_LEN},
};
use crate::diagnostics::Diagnostics;

const MAX_PAGES: usize = 128;
const MAX_KEYS: usize = 256;
const MAX_VALUE_LEN: usize = 2048;
const BUFFER_LEN: usize = KEY_LEN + VERSION_LEN + MAX_VALUE_LEN;

type SettingsCache = Cache<
    ArrayPageStates<MAX_PAGES>,
    ArrayPagePointers<MAX_PAGES>,
    ArrayKeyPointers<RecordKey, MAX_KEYS>,
    RecordKey,
>;

type Map<F> = MapStorage<RecordKey, Partition<F>, SettingsCache>;

pub struct Settings<F: MultiwriteNorFlash> {
    map: Option<Map<F>>,
    len: u32,
    buffer: Buffer,
    diagnostics: Option<&'static Diagnostics>,
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
    Unencodable,
    #[error("the settings partition holds fewer than two or more than {MAX_PAGES} sectors")]
    SectorCountOutOfRange,
    #[error("the settings store reported an internal error")]
    Internal,
}

#[repr(align(4))]
struct Buffer([u8; BUFFER_LEN]);

impl<F: MultiwriteNorFlash> Settings<F> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            map: None,
            len: 0,
            buffer: Buffer([0; BUFFER_LEN]),
            diagnostics: None,
        }
    }

    pub const fn with_diagnostics(&mut self, diagnostics: &'static Diagnostics) -> &mut Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub async fn load<R: Record>(&mut self, scope: Scope) -> Result<Option<R>, Error> {
        let loaded = self.fetch(scope).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&loaded);
        }
        loaded
    }

    pub async fn store<R: Record>(&mut self, scope: Scope, record: &R) -> Result<(), Error> {
        let stored = self.write(scope, record).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&stored);
        }
        stored
    }

    pub async fn remove(&mut self, key: RecordKey) -> Result<(), Error> {
        let removed = self.delete(key).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&removed);
        }
        removed
    }

    pub async fn visit(&mut self, visit: impl FnMut(RecordKey, Stored<'_>)) -> Result<(), Error> {
        let visited = self.walk(visit).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&visited);
        }
        visited
    }

    pub async fn load_with<T>(
        &mut self,
        key: RecordKey,
        read: impl FnOnce(Stored<'_>) -> T,
    ) -> Result<Option<T>, Error> {
        let loaded = self.fetch_with(key, read).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&loaded);
        }
        loaded
    }

    pub async fn store_with(
        &mut self,
        key: RecordKey,
        version: u16,
        encode: impl Fn(&mut [u8]) -> Option<usize>,
    ) -> Result<(), Error> {
        let stored = self.write_with(key, Encoder { version, encode }).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&stored);
        }
        stored
    }

    pub fn release(&mut self) -> Option<Partition<F>> {
        self.map.take().map(|map| map.destroy().0)
    }

    pub(super) fn attach(&mut self, partition: Partition<F>) -> Result<(), Error> {
        let len = partition.len();
        let pages = usize::try_from(len).unwrap_or(usize::MAX) / F::ERASE_SIZE;
        if pages > MAX_PAGES {
            return Err(Error::SectorCountOutOfRange);
        }
        self.map = Some(MapStorage::new(partition, config(len)?, cache()));
        self.len = len;
        Ok(())
    }

    pub(super) async fn erase(&mut self) -> Result<(), Error> {
        let (mut partition, _) = self.map.take().ok_or(Error::Internal)?.destroy();
        let erased = partition.erase(0, self.len).await;
        self.map = Some(MapStorage::new(partition, config(self.len)?, cache()));
        erased.map_err(|error| Error::Flash(error.kind()))
    }

    async fn fetch<R: Record>(&mut self, scope: Scope) -> Result<Option<R>, Error> {
        Ok(self
            .fetch_with(RecordKey::of::<R>(scope), |stored| stored.decode())
            .await?
            .flatten())
    }

    async fn write<R: Record>(&mut self, scope: Scope, record: &R) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .store_item(&mut buffer.0, &RecordKey::of::<R>(scope), &Encoding(record))
            .await
            .map_err(Error::from_store)
    }

    async fn delete(&mut self, key: RecordKey) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .remove_item(&mut buffer.0, &key)
            .await
            .map_err(Error::from_store)
    }

    async fn fetch_with<T>(
        &mut self,
        key: RecordKey,
        read: impl FnOnce(Stored<'_>) -> T,
    ) -> Result<Option<T>, Error> {
        let Self { map, buffer, .. } = self;
        let stored = map
            .as_mut()
            .ok_or(Error::Internal)?
            .fetch_item::<Stored<'_>>(&mut buffer.0, &key)
            .await
            .map_err(Error::from_store)?;
        Ok(stored.map(read))
    }

    async fn write_with<E: Fn(&mut [u8]) -> Option<usize>>(
        &mut self,
        key: RecordKey,
        encoder: Encoder<E>,
    ) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .store_item(&mut buffer.0, &key, &encoder)
            .await
            .map_err(Error::from_store)
    }

    async fn walk(&mut self, mut visit: impl FnMut(RecordKey, Stored<'_>)) -> Result<(), Error> {
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
}

impl<F: MultiwriteNorFlash> Default for Settings<F> {
    fn default() -> Self {
        Self::new()
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
            store::Error::SerializationError(_) => Self::Unencodable,
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
    MapConfig::try_new(0..len).map_err(|_| Error::SectorCountOutOfRange)
}
