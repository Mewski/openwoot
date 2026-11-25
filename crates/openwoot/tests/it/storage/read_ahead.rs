use embassy_futures::block_on;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use openwoot::storage::{Opened, Partition, Scope, Settings};

use super::{
    model::SECTOR,
    probe::Probe,
    support::{Blob, Brightness, PROFILES, SETTINGS_SECTORS, blob, chip, close, open},
};

const WIDE_SECTORS: u32 = 16;
const PROFILE_SWITCHES: u32 = 2440;
const OPENING_READS: u32 = 400;

#[test]
fn a_store_with_thousands_of_superseded_records_opens_in_a_few_hundred_flash_reads() {
    let (nor, layout) = chip(WIDE_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let blobs: Vec<Blob> = (0..u32::from(PROFILES)).map(blob).collect();
    for (profile, value) in (0..PROFILES).zip(&blobs) {
        block_on(settings.store(Scope::Profile(profile), value)).unwrap();
    }
    for switch in 0..PROFILE_SWITCHES {
        let [level, ..] = switch.to_le_bytes();
        block_on(settings.store(Scope::Global, &Brightness(level))).unwrap();
    }
    let (probe, watch) = Probe::new(close(settings).unwrap());
    let mut settings = Settings::new();
    let opened = block_on(settings.open(Partition::new(probe, layout.settings_region()), false));
    assert_eq!(opened, Ok(Opened::Kept));
    let reads = watch.reads();
    assert!(reads < OPENING_READS, "{reads} reads");
    for (profile, value) in (0..PROFILES).zip(&blobs) {
        assert_eq!(
            block_on(settings.load::<Blob>(Scope::Profile(profile))).unwrap(),
            Some(*value)
        );
    }
    let [last, ..] = (PROFILE_SWITCHES - 1).to_le_bytes();
    assert_eq!(
        block_on(settings.load::<Brightness>(Scope::Global)).unwrap(),
        Some(Brightness(last))
    );
}

#[test]
fn reads_behind_one_another_share_one_flash_read_and_a_repeated_read_reads_the_flash_again() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (probe, watch) = Probe::new(nor);
    let mut partition = Partition::new(probe, layout.settings_region());
    block_on(partition.write(0, &[1, 2, 3, 4, 5, 6, 7, 8])).unwrap();
    let mut first = [0; 4];
    let mut second = [0; 4];
    block_on(partition.read(0, &mut first)).unwrap();
    block_on(partition.read(4, &mut second)).unwrap();
    assert_eq!((first, second), ([1, 2, 3, 4], [5, 6, 7, 8]));
    assert_eq!(watch.reads(), 1);
    block_on(partition.read(4, &mut second)).unwrap();
    assert_eq!(watch.reads(), 2);
    block_on(partition.read(SECTOR, &mut first)).unwrap();
    assert_eq!(watch.reads(), 3);
}

#[test]
fn a_write_or_an_erase_drops_what_was_read_ahead() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let mut partition = Partition::new(nor, layout.settings_region());
    let mut bytes = [0; 4];
    block_on(partition.read(0, &mut bytes)).unwrap();
    block_on(partition.write(8, &[1, 2, 3, 4])).unwrap();
    block_on(partition.read(8, &mut bytes)).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
    block_on(partition.read(12, &mut bytes)).unwrap();
    block_on(partition.erase(0, SECTOR)).unwrap();
    block_on(partition.read(8, &mut bytes)).unwrap();
    assert_eq!(bytes, [0xFF; 4]);
}

#[test]
fn a_new_partition_reads_the_flash_afresh() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let mut partition = Partition::new(nor, layout.settings_region());
    let mut bytes = [0; 4];
    block_on(partition.read(0, &mut bytes)).unwrap();
    let mut nor = partition.release();
    block_on(nor.write(4, &[1, 2, 3, 4])).unwrap();
    let mut partition = Partition::new(nor, layout.settings_region());
    block_on(partition.read(4, &mut bytes)).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
}

#[test]
fn reads_at_the_end_of_the_partition_stay_inside_it() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let mut partition = Partition::new(nor, layout.settings_region());
    let end = SETTINGS_SECTORS * SECTOR;
    block_on(partition.write(end - 4, &[1, 2, 3, 4])).unwrap();
    let mut bytes = [0; 4];
    block_on(partition.read(end - 4, &mut bytes)).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4]);
    assert!(block_on(partition.read(end - 2, &mut bytes)).is_err());
}

#[test]
fn a_read_longer_than_the_read_ahead_reads_the_flash_once() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (probe, watch) = Probe::new(nor);
    let mut partition = Partition::new(probe, layout.settings_region());
    block_on(partition.write(SECTOR - 4, &[1, 2, 3, 4])).unwrap();
    let mut sector = vec![0; SECTOR as usize];
    block_on(partition.read(0, &mut sector)).unwrap();
    assert_eq!(watch.reads(), 1);
    assert_eq!(sector[SECTOR as usize - 4..], [1, 2, 3, 4]);
}
