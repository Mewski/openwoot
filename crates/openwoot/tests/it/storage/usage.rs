use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use openwoot::{
    keys,
    profile::{self, Model},
    storage::{Error, Partition, RecordKey, Scope, Settings},
};
use openwoot_protocol::{configuration::Rejection, diagnostics::StorageUsage};

use super::{
    model::{Nor, SECTOR},
    probe::Probe,
    support::{Blob, Brightness, PROFILES, SETTINGS_SECTORS, blob, chip, close, filled, open},
};
use crate::{
    lighting::{COLUMNS, ROWS},
    profile::FACTORY,
};

const HEADER: u32 = 8;
const KEY_AND_VERSION: u32 = 3;
const ONE_BYTE_ITEM: u32 = HEADER + KEY_AND_VERSION + 1;
const BLOB_ITEM: u32 = HEADER + KEY_AND_VERSION + 1 + 6 * 32;
const ITEM_BYTES_PER_SECTOR: u32 = SECTOR - 8;

#[test]
fn usage_counts_every_stored_item_and_tells_the_current_records_apart() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let capacity = SETTINGS_SECTORS * SECTOR;
    assert_eq!(
        block_on(settings.usage()),
        Ok(StorageUsage {
            capacity,
            used: ONE_BYTE_ITEM,
            records: 1,
            items: 1,
        })
    );
    block_on(settings.store(Scope::Global, &Brightness(1))).unwrap();
    block_on(settings.store(Scope::Global, &Brightness(2))).unwrap();
    block_on(settings.store(Scope::Profile(0), &blob(1))).unwrap();
    block_on(settings.store(Scope::Profile(0), &blob(2))).unwrap();
    block_on(settings.store(Scope::Profile(1), &blob(3))).unwrap();
    let stored = StorageUsage {
        capacity,
        used: 3 * ONE_BYTE_ITEM + 3 * BLOB_ITEM,
        records: 4,
        items: 6,
    };
    assert_eq!(block_on(settings.usage()), Ok(stored));
    let (mut settings, _) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(block_on(settings.usage()), Ok(stored));
    block_on(settings.remove(RecordKey::of::<Blob>(Scope::Profile(1)))).unwrap();
    assert_eq!(
        block_on(settings.usage()),
        Ok(StorageUsage {
            used: stored.used - BLOB_ITEM,
            records: 3,
            items: 5,
            ..stored
        })
    );
}

#[test]
fn superseded_items_leave_the_usage_once_garbage_collection_erases_their_sector() {
    let updates = 150;
    let (nor, layout, _) = filled(updates).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let usage = block_on(settings.usage()).unwrap();
    assert_eq!(usage.records, u32::from(PROFILES) + 1);
    assert!(usage.items < updates, "{usage:?}");
    assert!(usage.items > usage.records, "{usage:?}");
    assert!(
        usage.used <= (SETTINGS_SECTORS - 1) * ITEM_BYTES_PER_SECTOR,
        "{usage:?}"
    );
}

#[test]
fn usage_of_a_flash_that_cannot_be_read_is_a_flash_error() {
    let (nor, layout, _) = filled(20).unwrap();
    let (probe, watch) = Probe::new(nor);
    let mut settings = Settings::new();
    block_on(settings.open(Partition::new(probe, layout.settings_region()), false)).unwrap();
    watch.fail_reads(true);
    assert!(
        matches!(block_on(settings.usage()), Err(Error::Flash(_))),
        "a failed read reports the flash"
    );
}

#[test]
fn the_profile_model_answers_unsupported_without_a_store_and_inaccessible_when_reads_fail() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut storeless = Model::<CriticalSectionRawMutex, Nor, ROWS, COLUMNS>::new(
        &keys, &shared, None, &FACTORY, 0, 4,
    );
    assert_eq!(
        block_on(storeless.storage_usage()),
        Err(Rejection::Unsupported)
    );
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (probe, watch) = Probe::new(nor);
    let mut settings = Settings::new();
    block_on(settings.open(Partition::new(probe, layout.settings_region()), false)).unwrap();
    let mut model = Model::<CriticalSectionRawMutex, Probe, ROWS, COLUMNS>::new(
        &keys,
        &shared,
        Some(&mut settings),
        &FACTORY,
        0,
        4,
    );
    assert_eq!(
        block_on(model.storage_usage()).map(|usage| usage.records),
        Ok(1)
    );
    watch.fail_reads(true);
    assert_eq!(
        block_on(model.storage_usage()),
        Err(Rejection::StorageInaccessible)
    );
}
