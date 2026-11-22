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
    record::{Encoding, KEY_LEN, Key, Record, Scope, Stored, VERSION_LEN},
};
use crate::diagnostics::Diagnostics;

pub const MAX_PAGES: usize = 128;
pub const MAX_KEYS: usize = 256;
pub const MAX_VALUE_LEN: usize = 2048;

const BUFFER_LEN: usize = KEY_LEN + VERSION_LEN + MAX_VALUE_LEN;

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
    Encoding,
    #[error("the settings partition holds fewer than two or more than {MAX_PAGES} sectors")]
    Layout,
    #[error("the settings store reported an internal error")]
    Internal,
}

#[repr(align(4))]
struct Buffer([u8; BUFFER_LEN]);

impl<F: MultiwriteNorFlash> Settings<F> {
    #[must_use]
    pub const fn with_diagnostics(mut self, diagnostics: &'static Diagnostics) -> Self {
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

    pub async fn remove(&mut self, key: Key) -> Result<(), Error> {
        let removed = self.delete(key).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&removed);
        }
        removed
    }

    pub async fn visit(&mut self, visit: impl FnMut(Key, Stored<'_>)) -> Result<(), Error> {
        let visited = self.walk(visit).await;
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&visited);
        }
        visited
    }

    pub fn release(self) -> Option<Partition<F>> {
        self.map.map(|map| map.destroy().0)
    }

    pub(super) fn new(partition: Partition<F>) -> Result<Self, Error> {
        let len = partition.len();
        let pages = usize::try_from(len).unwrap_or(usize::MAX) / F::ERASE_SIZE;
        if pages > MAX_PAGES {
            return Err(Error::Layout);
        }
        Ok(Self {
            map: Some(MapStorage::new(partition, config(len)?, cache())),
            len,
            buffer: Buffer([0; BUFFER_LEN]),
            diagnostics: None,
        })
    }

    pub(super) async fn erase(&mut self) -> Result<(), Error> {
        let (mut partition, _) = self.map.take().ok_or(Error::Internal)?.destroy();
        let erased = partition.erase(0, self.len).await;
        self.map = Some(MapStorage::new(partition, config(self.len)?, cache()));
        erased.map_err(|error| Error::Flash(error.kind()))
    }

    async fn fetch<R: Record>(&mut self, scope: Scope) -> Result<Option<R>, Error> {
        let Self { map, buffer, .. } = self;
        let stored = map
            .as_mut()
            .ok_or(Error::Internal)?
            .fetch_item::<Stored<'_>>(&mut buffer.0, &Key::of::<R>(scope))
            .await
            .map_err(Error::from_store)?;
        Ok(stored.and_then(|stored| stored.decode()))
    }

    async fn write<R: Record>(&mut self, scope: Scope, record: &R) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .store_item(&mut buffer.0, &Key::of::<R>(scope), &Encoding(record))
            .await
            .map_err(Error::from_store)
    }

    async fn delete(&mut self, key: Key) -> Result<(), Error> {
        let Self { map, buffer, .. } = self;
        map.as_mut()
            .ok_or(Error::Internal)?
            .remove_item(&mut buffer.0, &key)
            .await
            .map_err(Error::from_store)
    }

    async fn walk(&mut self, mut visit: impl FnMut(Key, Stored<'_>)) -> Result<(), Error> {
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
