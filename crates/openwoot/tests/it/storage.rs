mod format;
mod model;
mod power_cut;
mod records;
mod sectors;

use embassy_futures::block_on;
use openwoot::storage::{self, NorLayout, Opened, Partition, Record, Scope, Settings};
use openwoot_image::Region;
use serde::{Deserialize, Serialize};

pub(crate) use self::model::{Nor, SECTOR};

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

impl Record for Blob {
    const ITEM: u8 = 1;
    const VERSION: u16 = 1;
}

impl Record for Brightness {
    const ITEM: u8 = 2;
    const VERSION: u16 = 1;
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
