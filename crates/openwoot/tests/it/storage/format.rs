use embassy_futures::block_on;
use openwoot::storage::{FormatReason, Opened, Scope};

use super::{
    Blob, Brightness, SETTINGS_SECTORS, blob, chip, close, device_info_untouched, filled, open,
};

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
