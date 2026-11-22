mod model;

use embassy_futures::block_on;
use openwoot::storage::{
    self, FormatReason, Key, NorLayout, Opened, Partition, Record, Scope, Settings,
};
use openwoot_image::Region;
use serde::{Deserialize, Serialize};

use self::model::{Nor, SECTOR};

const SETTINGS_SECTORS: u32 = 4;
const PROFILES: u8 = 6;
const DEVICE_INFO_PATTERN: u8 = 0x5A;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Blob {
    seed: u32,
    bytes: [[u8; 32]; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Brightness(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Lighting {
    brightness: u8,
    effect: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FutureLighting {
    brightness: u8,
    effect: u8,
    speed: u16,
}

impl Record for Blob {
    const ITEM: u8 = 1;
    const VERSION: u16 = 1;
}

impl Record for Brightness {
    const ITEM: u8 = 2;
    const VERSION: u16 = 1;
}

impl Record for Lighting {
    const ITEM: u8 = 2;
    const VERSION: u16 = 2;

    fn migrate(version: u16, body: &[u8]) -> Option<Self> {
        match version {
            1 => postcard::from_bytes::<Brightness>(body)
                .ok()
                .map(|Brightness(brightness)| Self {
                    brightness,
                    effect: 0,
                }),
            _ => None,
        }
    }
}

impl Record for FutureLighting {
    const ITEM: u8 = 2;
    const VERSION: u16 = 3;
}

fn blob(seed: u32) -> Blob {
    let [low, ..] = seed.to_le_bytes();
    let mut value = low.wrapping_mul(31);
    let mut bytes = [[0; 32]; 6];
    for byte in bytes.iter_mut().flatten() {
        value = value.wrapping_add(7);
        *byte = value;
    }
    Blob { seed, bytes }
}

fn profile_of(update: u32) -> u8 {
    u8::try_from(update % u32::from(PROFILES)).unwrap_or_default()
}

fn chip(settings_sectors: u32) -> (Nor, NorLayout) {
    let sectors = settings_sectors + 2;
    let mut nor = Nor::new(sectors);
    nor.sector_mut(sectors - 1).fill(DEVICE_INFO_PATTERN);
    let layout = NorLayout {
        capacity: sectors * SECTOR,
        settings: Region {
            start: 0,
            end: settings_sectors * SECTOR,
        },
        device_info: Some(Region {
            start: (sectors - 1) * SECTOR,
            end: sectors * SECTOR,
        }),
    };
    (nor, layout)
}

fn open(
    nor: Nor,
    layout: NorLayout,
    format_requested: bool,
) -> Result<(Settings<Nor>, Opened), storage::Error> {
    let partition = Partition::new(nor, layout.settings_region());
    block_on(Settings::open(partition, format_requested))
}

fn close(settings: Settings<Nor>) -> Option<Nor> {
    settings.release().map(Partition::release)
}

fn device_info_untouched(nor: &Nor) -> bool {
    let sectors = u32::try_from(nor.erases().len()).unwrap_or_default();
    nor.sector(sectors.saturating_sub(1))
        .iter()
        .all(|byte| *byte == DEVICE_INFO_PATTERN)
        && nor.erases().iter().rev().take(2).all(|erases| *erases == 0)
}

fn filled(updates: u32) -> Result<(Nor, NorLayout, Vec<Blob>), storage::Error> {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false)?;
    let mut expected: Vec<Blob> = (0..u32::from(PROFILES)).map(blob).collect();
    for (profile, value) in (0..PROFILES).zip(&expected) {
        block_on(settings.store(Scope::Profile(profile), value))?;
    }
    for update in 0..updates {
        let profile = profile_of(update);
        let value = blob(100 + update);
        block_on(settings.store(Scope::Profile(profile), &value))?;
        if let Some(slot) = expected.get_mut(usize::from(profile)) {
            *slot = value;
        }
    }
    let nor = close(settings).ok_or(storage::Error::Internal)?;
    Ok((nor, layout, expected))
}

#[test]
fn a_blank_chip_is_formatted_once_and_keeps_values_across_a_restart() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, opened) = open(nor, layout, false).unwrap();
    assert_eq!(opened, Opened::Formatted(FormatReason::Unrecognised));
    block_on(settings.store(Scope::Global, &Brightness(80))).unwrap();
    block_on(settings.store(Scope::Profile(2), &blob(7))).unwrap();
    let (mut settings, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
    assert_eq!(
        block_on(settings.load::<Brightness>(Scope::Global)).unwrap(),
        Some(Brightness(80))
    );
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(2))).unwrap(),
        Some(blob(7))
    );
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(3))).unwrap(),
        None
    );
    assert!(device_info_untouched(&close(settings).unwrap()));
}

#[test]
fn foreign_content_such_as_a_stock_volume_is_replaced() {
    let (mut nor, layout) = chip(SETTINGS_SECTORS);
    let mut state = 0x1234_5678u32;
    for sector in 0..SETTINGS_SECTORS {
        for byte in nor.sector_mut(sector) {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            [*byte, ..] = state.to_be_bytes();
        }
    }
    let (settings, opened) = open(nor, layout, false).unwrap();
    assert_eq!(opened, Opened::Formatted(FormatReason::Unrecognised));
    let (_, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
}

#[test]
fn a_format_request_and_a_factory_reset_drop_every_setting() {
    let (nor, layout, _) = filled(10).unwrap();
    let (mut settings, opened) = open(nor, layout, true).unwrap();
    assert_eq!(opened, Opened::Formatted(FormatReason::Requested));
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(0))).unwrap(),
        None
    );
    block_on(settings.store(Scope::Profile(1), &blob(3))).unwrap();
    block_on(settings.factory_reset()).unwrap();
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(1))).unwrap(),
        None
    );
    let nor = close(settings).unwrap();
    assert!(device_info_untouched(&nor));
    let (_, opened) = open(nor, layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
}

#[test]
fn visiting_reports_every_key_and_the_last_value_wins() {
    let (nor, layout, expected) = filled(20).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    block_on(settings.remove(Key::of::<Blob>(Scope::Profile(5)))).unwrap();
    let mut seen = vec![None; usize::from(PROFILES)];
    block_on(settings.visit(|key, stored| {
        if let (Scope::Profile(profile), Blob::ITEM) = (key.scope(), key.item()) {
            seen[usize::from(profile)] = stored.decode::<Blob>();
        }
    }))
    .unwrap();
    let mut wanted: Vec<Option<Blob>> = expected.into_iter().map(Some).collect();
    wanted[5] = None;
    assert_eq!(seen, wanted);
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(5))).unwrap(),
        None
    );
}

#[test]
fn older_records_migrate_and_newer_records_count_as_absent() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    block_on(settings.store(Scope::Global, &Brightness(40))).unwrap();
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Global)).unwrap(),
        Some(Lighting {
            brightness: 40,
            effect: 0
        })
    );
    block_on(settings.store(
        Scope::Global,
        &FutureLighting {
            brightness: 1,
            effect: 2,
            speed: 3,
        },
    ))
    .unwrap();
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Global)).unwrap(),
        None
    );
}

#[test]
fn a_power_cut_at_any_step_of_a_store_keeps_every_other_setting() {
    let (mut base, layout, mut expected) = filled(0).unwrap();
    for updates in 0..150 {
        let target = profile_of(updates);
        let new = blob(10_000 + updates);
        for cut in 0.. {
            let (mut settings, _) = open(base.cut_after(cut), layout, false).unwrap();
            let stored = block_on(settings.store(Scope::Profile(target), &new));
            let nor = close(settings).unwrap();
            if !nor.was_cut() {
                stored.unwrap();
                break;
            }
            let (mut settings, opened) = open(nor.revived(), layout, false).unwrap();
            assert_eq!(opened, Opened::Kept, "update {updates}, cut {cut}");
            for profile in 0..PROFILES {
                let loaded = block_on(settings.load::<Blob>(Scope::Profile(profile))).unwrap();
                let old = expected[usize::from(profile)];
                if profile == target {
                    assert!(
                        loaded == Some(old) || loaded == Some(new),
                        "update {updates}, cut {cut}"
                    );
                } else {
                    assert_eq!(
                        loaded,
                        Some(old),
                        "update {updates}, cut {cut}, profile {profile}"
                    );
                }
            }
            block_on(settings.store(Scope::Profile(target), &new)).unwrap();
            assert_eq!(
                block_on(settings.load::<Blob>(Scope::Profile(target))).unwrap(),
                Some(new)
            );
            assert!(device_info_untouched(&close(settings).unwrap()));
        }
        let (mut settings, _) = open(base, layout, false).unwrap();
        block_on(settings.store(Scope::Profile(target), &new)).unwrap();
        expected[usize::from(target)] = new;
        base = close(settings).unwrap();
    }
    assert!(
        base.erases().iter().sum::<u32>() >= 2 * SETTINGS_SECTORS,
        "{:?}",
        base.erases()
    );
}

#[test]
fn a_power_cut_during_a_factory_reset_leaves_a_working_store() {
    let (base, layout, _) = filled(30).unwrap();
    for cut in 0.. {
        let (mut settings, _) = open(base.cut_after(cut), layout, false).unwrap();
        let reset = block_on(settings.factory_reset());
        let nor = close(settings).unwrap();
        if !nor.was_cut() {
            reset.unwrap();
            break;
        }
        let (mut settings, _) = open(nor.revived(), layout, false).unwrap();
        block_on(settings.store(Scope::Profile(0), &blob(1))).unwrap();
        assert_eq!(
            block_on(settings.load::<Blob>(Scope::Profile(0))).unwrap(),
            Some(blob(1))
        );
        assert!(device_info_untouched(&close(settings).unwrap()));
    }
}

#[test]
fn wear_spreads_evenly_over_the_settings_sectors() {
    let settings_sectors = 8;
    let (nor, layout) = chip(settings_sectors);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    for update in 0..3000u32 {
        let profile = profile_of(update);
        block_on(settings.store(Scope::Profile(profile), &blob(update))).unwrap();
    }
    let nor = close(settings).unwrap();
    let wear = &nor.erases()[..8];
    let least = *wear.iter().min().unwrap();
    let most = *wear.iter().max().unwrap();
    assert!(least > 0, "{wear:?}");
    assert!(most - least <= 1, "{wear:?}");
    assert!(device_info_untouched(&nor));
}

#[test]
fn the_partition_refuses_access_outside_its_sectors() {
    use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};

    let (nor, layout) = chip(SETTINGS_SECTORS);
    let mut partition = Partition::new(nor, layout.settings_region());
    let end = SETTINGS_SECTORS * SECTOR;
    assert!(block_on(partition.erase(end, end + SECTOR)).is_err());
    assert!(block_on(partition.write(end - 4, &[0; 8])).is_err());
    assert!(block_on(partition.read(end, &mut [0; 1])).is_err());
    assert!(block_on(partition.erase(0, SECTOR)).is_ok());
    assert!(device_info_untouched(&partition.release()));
}
