use embassy_futures::block_on;
use embedded_storage_async::nor_flash::{NorFlash, ReadNorFlash};
use openwoot::storage::{Partition, Scope};

use super::{
    SETTINGS_SECTORS, blob, chip, close, device_info_untouched, model::SECTOR, open, profile_of,
};

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
