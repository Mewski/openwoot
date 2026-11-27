use core::fmt;

use embassy_futures::block_on;
use embedded_storage_async::nor_flash::{
    MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind,
};
use openwoot_protocol::{diagnostics::StorageUsage, wire};
use sequential_storage::{
    self as store,
    cache::{
        Cache, key_pointers::ArrayKeyPointers, page_pointers::ArrayPagePointers,
        page_states::ArrayPageStates,
    },
    map::{MapConfig, MapStorage},
};

use super::{
    nor_layout::MAXIMUM_SETTINGS_SECTORS,
    partition::{Partition, PartitionError},
    record::{Encoder, KEY_LEN, MAX_VALUE_LEN, Record, RecordKey, Scope, Stored, VERSION_LEN},
    stored_keys::StoredKeys,
    usage::Tally,
};
use crate::diagnostics::Diagnostics;

const MAX_PAGES: usize = MAXIMUM_SETTINGS_SECTORS as usize;
const MAX_KEYS: usize = 256;
const WORD_LEN: usize = 4;
const BUFFER_LEN: usize = (KEY_LEN + VERSION_LEN + MAX_VALUE_LEN).next_multiple_of(WORD_LEN);

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
    known: StoredKeys,
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
            known: StoredKeys::unknown(),
            diagnostics: None,
        }
    }

    pub const fn with_diagnostics(&mut self, diagnostics: &'static Diagnostics) -> &mut Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    pub fn load<R: Record>(&mut self, scope: Scope) -> Result<Option<R>, Error> {
        Ok(self
            .load_stored(RecordKey::of::<R>(scope))?
            .and_then(|stored| stored.decode()))
    }

    pub fn store<R: Record>(&mut self, scope: Scope, record: &R) -> Result<(), Error> {
        self.store_with(RecordKey::of::<R>(scope), R::VERSION, &|body| {
            wire::encode(record, body)
        })
    }

    pub fn remove(&mut self, key: RecordKey) -> Result<(), Error> {
        let removed = self.delete(key);
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&removed);
        }
        removed
    }

    pub fn visit(&mut self, visit: &mut dyn FnMut(RecordKey, Stored<'_>)) -> Result<(), Error> {
        let visited = Self::walk(&mut self.map, &mut self.buffer, visit);
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&visited);
        }
        visited
    }

    pub fn usage(&mut self) -> Result<StorageUsage, Error> {
        let mut tally = Tally::new(
            self.len,
            Map::<F>::item_overhead_size(),
            F::WRITE_SIZE.max(F::READ_SIZE),
        );
        let indexed = self.index(&mut |_, stored| tally.add(stored));
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_loaded(&indexed);
        }
        indexed?;
        Ok(tally.finish(self.known.count()))
    }

    pub fn load_stored(&mut self, key: RecordKey) -> Result<Option<Stored<'_>>, Error> {
        let diagnostics = self.diagnostics;
        let loaded = self.fetch(key);
        if let Some(diagnostics) = diagnostics {
            diagnostics.storage_loaded(&loaded);
        }
        loaded
    }

    pub fn store_with(
        &mut self,
        key: RecordKey,
        version: u8,
        encode: &dyn Fn(&mut [u8]) -> Option<usize>,
    ) -> Result<(), Error> {
        let stored = self.write(key, &Encoder { version, encode });
        if let Some(diagnostics) = self.diagnostics {
            diagnostics.storage_stored(&stored);
        }
        stored
    }

    pub fn release(&mut self) -> Option<Partition<F>> {
        self.known.forget();
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
        self.known.forget();
        Ok(())
    }

    pub(super) fn erase(&mut self) -> Result<(), Error> {
        let (mut partition, _) = self.map.take().ok_or(Error::Internal)?.destroy();
        let erased = block_on(partition.erase(0, self.len));
        self.map = Some(MapStorage::new(partition, config(self.len)?, cache()));
        if erased.is_ok() {
            self.known.start();
        } else {
            self.known.forget();
        }
        erased.map_err(|error| Error::Flash(error.kind()))
    }

    pub(super) fn index(
        &mut self,
        visit: &mut dyn FnMut(RecordKey, Stored<'_>),
    ) -> Result<(), Error> {
        let Self {
            map, buffer, known, ..
        } = self;
        known.start();
        let indexed = Self::walk(map, buffer, &mut |key, stored| {
            known.insert(key);
            visit(key, stored);
        });
        if indexed.is_err() {
            known.forget();
        }
        indexed
    }

    fn fetch(&mut self, key: RecordKey) -> Result<Option<Stored<'_>>, Error> {
        let Self {
            map, buffer, known, ..
        } = self;
        if known.lacks(key) {
            return Ok(None);
        }
        let stored = block_on(
            map.as_mut()
                .ok_or(Error::Internal)?
                .fetch_item::<Stored<'_>>(&mut buffer.0, &key),
        )
        .map_err(Error::from_store)?;
        if stored.is_none() {
            known.remove(key);
        }
        Ok(stored)
    }

    fn write(&mut self, key: RecordKey, encoder: &Encoder<'_>) -> Result<(), Error> {
        let Self {
            map, buffer, known, ..
        } = self;
        known.insert(key);
        block_on(
            map.as_mut()
                .ok_or(Error::Internal)?
                .store_item(&mut buffer.0, &key, encoder),
        )
        .map_err(Error::from_store)
    }

    fn delete(&mut self, key: RecordKey) -> Result<(), Error> {
        let Self {
            map, buffer, known, ..
        } = self;
        if known.lacks(key) {
            return Ok(());
        }
        block_on(
            map.as_mut()
                .ok_or(Error::Internal)?
                .remove_item(&mut buffer.0, &key),
        )
        .map_err(Error::from_store)?;
        known.remove(key);
        Ok(())
    }

    fn walk(
        map: &mut Option<Map<F>>,
        buffer: &mut Buffer,
        visit: &mut dyn FnMut(RecordKey, Stored<'_>),
    ) -> Result<(), Error> {
        let map = map.as_mut().ok_or(Error::Internal)?;
        block_on(async {
            let mut items = map
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
        })
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
