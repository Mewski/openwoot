use embassy_futures::block_on;
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    storage::{Opened, Partition, Record, RecordKey, Scope, Settings},
};
use openwoot_protocol::diagnostics::{Storage, Usb};
use serde::{Deserialize, Serialize};

use super::{
    probe::Probe,
    support::{Blob, Brightness, PROFILES, SETTINGS_SECTORS, chip, close, filled, open},
};

const LARGEST_BODY: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Lighting {
    brightness: u8,
    effect: u8,
}

impl Record for Lighting {
    const ITEM: u8 = 2;
    const VERSION: u8 = 2;
}

#[test]
fn visiting_reports_every_key_and_the_last_value_wins() {
    let (nor, layout, expected) = filled(20).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    block_on(settings.remove(RecordKey::of::<Blob>(Scope::Profile(5)))).unwrap();
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
fn a_record_of_another_version_counts_as_absent() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    block_on(settings.store(Scope::Global, &Brightness(40))).unwrap();
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Global)).unwrap(),
        None
    );
    block_on(settings.store(
        Scope::Global,
        &Lighting {
            brightness: 1,
            effect: 2,
        },
    ))
    .unwrap();
    assert_eq!(
        block_on(settings.load::<Brightness>(Scope::Global)).unwrap(),
        None
    );
}

#[test]
fn settings_count_their_loads_and_stores_into_the_diagnostics() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut opened, _) = open(nor, layout, false).unwrap();
    let settings = opened.with_diagnostics(&DIAGNOSTICS);
    block_on(settings.store(Scope::Global, &Brightness(40))).unwrap();
    block_on(settings.load::<Brightness>(Scope::Global)).unwrap();
    block_on(settings.remove(RecordKey::of::<Brightness>(Scope::Global))).unwrap();
    assert_eq!(
        DIAGNOSTICS
            .report(Usb::default(), Hardware::default())
            .storage,
        Storage {
            loads: 1,
            stores: 2,
            failures: 0,
        }
    );
}

#[test]
fn records_encoded_and_decoded_in_place_are_ordinary_records() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let key = RecordKey::of::<Lighting>(Scope::Profile(1));
    let lighting = Lighting {
        brightness: 7,
        effect: 3,
    };
    block_on(settings.store_with(key, Lighting::VERSION, |body| {
        postcard::to_slice(&lighting, body)
            .ok()
            .map(|body| body.len())
    }))
    .unwrap();
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Profile(1))).unwrap(),
        Some(lighting)
    );
    assert_eq!(
        block_on(settings.load_with(key, |stored| stored.decode::<Lighting>())).unwrap(),
        Some(Some(lighting))
    );
    assert_eq!(
        block_on(
            settings.load_with(RecordKey::of::<Lighting>(Scope::Global), |stored| {
                stored.version
            })
        )
        .unwrap(),
        None
    );
}

#[test]
fn an_encoding_that_fails_or_overflows_keeps_the_stored_record() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let kept = Lighting {
        brightness: 9,
        effect: 1,
    };
    block_on(settings.store(Scope::Global, &kept)).unwrap();
    let key = RecordKey::of::<Lighting>(Scope::Global);
    assert_eq!(
        block_on(settings.store_with(key, Lighting::VERSION, |_| None)),
        Err(openwoot::storage::Error::Unencodable)
    );
    assert!(
        block_on(settings.store_with(key, Lighting::VERSION, |body| Some(body.len() + 1))).is_err()
    );
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Global)).unwrap(),
        Some(kept)
    );
}

#[test]
fn a_key_that_was_never_stored_is_answered_without_reading_the_flash() {
    let (nor, layout, _) = filled(20).unwrap();
    let (probe, watch) = Probe::new(nor);
    let mut settings = Settings::new();
    let opened = block_on(settings.open(Partition::new(probe, layout.settings_region()), false));
    assert_eq!(opened, Ok(Opened::Kept));
    let absent = RecordKey::of::<Lighting>(Scope::Profile(9));
    let before = watch.reads();
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Profile(9))),
        Ok(None)
    );
    assert_eq!(block_on(settings.remove(absent)), Ok(()));
    assert_eq!(watch.reads(), before);
    assert!(
        block_on(settings.load::<Blob>(Scope::Profile(3)))
            .unwrap()
            .is_some()
    );
    assert!(watch.reads() > before);
}

#[test]
fn a_key_is_found_again_after_it_was_removed_and_stored_anew() {
    let (nor, layout, _) = filled(20).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let key = RecordKey::of::<Blob>(Scope::Profile(2));
    block_on(settings.remove(key)).unwrap();
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(2))).unwrap(),
        None
    );
    let lighting = Lighting {
        brightness: 3,
        effect: 4,
    };
    block_on(settings.store(Scope::Profile(2), &lighting)).unwrap();
    let (mut settings, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
    assert_eq!(
        block_on(settings.load::<Lighting>(Scope::Profile(2))).unwrap(),
        Some(lighting)
    );
    assert_eq!(
        block_on(settings.load::<Blob>(Scope::Profile(2))).unwrap(),
        None
    );
    assert!(
        block_on(settings.load::<Blob>(Scope::Profile(1)))
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_record_of_the_largest_size_is_stored_and_read_back() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let key = RecordKey::of::<Lighting>(Scope::Profile(1));
    for len in [LARGEST_BODY - 2, LARGEST_BODY - 1, LARGEST_BODY] {
        block_on(settings.store_with(key, Lighting::VERSION, |body| {
            body.get_mut(..len)?.fill(0xA5);
            Some(len)
        }))
        .unwrap();
        assert_eq!(
            block_on(settings.load_with(key, |stored| stored.body.len())),
            Ok(Some(len))
        );
    }
    assert!(
        block_on(settings.store_with(key, Lighting::VERSION, |body| {
            body.get_mut(..=LARGEST_BODY)?.fill(0xA5);
            Some(LARGEST_BODY + 1)
        }))
        .is_err()
    );
    assert_eq!(
        block_on(settings.load_with(key, |stored| stored.body.len())),
        Ok(Some(LARGEST_BODY))
    );
    let (mut settings, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
    assert_eq!(
        block_on(settings.load_with(key, |stored| stored.body.len())),
        Ok(Some(LARGEST_BODY))
    );
}
