use embassy_futures::block_on;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use openwoot::storage::{NorLayout, Partition, Scope, Settings};
use openwoot_image::Region;

use super::{
    model::{Nor, SECTOR},
    support::{Blob, SETTINGS_SECTORS, blob, chip, close, device_info_untouched, open, profile_of},
};

const NEIGHBOUR_PATTERN: u8 = 0x5A;

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
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let mut partition = Partition::new(nor, layout.settings_region());
    let end = SETTINGS_SECTORS * SECTOR;
    assert!(block_on(partition.erase(end, end + SECTOR)).is_err());
    assert!(block_on(partition.write(end - 4, &[0; 8])).is_err());
    assert!(block_on(partition.read(end, &mut [0; 1])).is_err());
    assert!(block_on(partition.erase(0, SECTOR)).is_ok());
    assert!(device_info_untouched(&partition.release()));
}

#[test]
fn a_partition_behind_other_sectors_leaves_its_neighbours_alone() {
    let neighbours = [0, 1, 6, 7];
    let mut nor = Nor::new(8);
    for sector in neighbours {
        nor.sector_mut(sector).fill(NEIGHBOUR_PATTERN);
    }
    let layout = NorLayout {
        capacity: 8 * SECTOR,
        settings: Region {
            start: 2 * SECTOR,
            end: 6 * SECTOR,
        },
        device_info: None,
    };
    let mut settings = Settings::new();
    block_on(settings.open(Partition::new(nor, layout.settings_region()), false)).unwrap();
    for update in 0..200 {
        let profile = profile_of(update);
        block_on(settings.store(Scope::Profile(profile), &blob(update))).unwrap();
    }
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(profile_of(199)))).unwrap(),
        Some(blob(199))
    );
    let nor = close(settings).unwrap();
    for sector in neighbours {
        assert!(
            nor.sector(sector)
                .iter()
                .all(|byte| *byte == NEIGHBOUR_PATTERN),
            "sector {sector}"
        );
    }
    let erases = nor.erases();
    assert!(erases[2..6].iter().all(|count| *count > 0), "{erases:?}");
    assert!(
        erases[..2]
            .iter()
            .chain(&erases[6..])
            .all(|count| *count == 0),
        "{erases:?}"
    );
}
