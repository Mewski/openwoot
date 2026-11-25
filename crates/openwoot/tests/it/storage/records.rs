use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    storage::{Error, Opened, Partition, Record, RecordKey, Scope, Settings},
};
use openwoot_protocol::{
    diagnostics::{Storage, Usb},
    heapless,
};
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Text(heapless::Vec<u8, 4096>);

impl Record for Text {
    const ITEM: u8 = 3;
    const VERSION: u8 = 1;
}

fn text(len: usize, byte: u8) -> Text {
    Text(core::iter::repeat_n(byte, len).collect())
}

#[test]
fn visiting_reports_every_key_and_the_last_value_wins() {
    let (nor, layout, expected) = filled(20).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    settings
        .remove(RecordKey::of::<Blob>(Scope::Profile(5)))
        .unwrap();
    let mut seen = vec![None; usize::from(PROFILES)];
    settings
        .visit(&mut |key, stored| {
            if let (Scope::Profile(profile), Blob::ITEM) = (key.scope(), key.item()) {
                seen[usize::from(profile)] = stored.decode::<Blob>();
            }
        })
        .unwrap();
    let mut wanted: Vec<Option<Blob>> = expected.into_iter().map(Some).collect();
    wanted[5] = None;
    assert_eq!(seen, wanted);
    assert_eq!(settings.load::<Blob>(Scope::Profile(5)).unwrap(), None);
}

#[test]
fn a_record_of_another_version_counts_as_absent() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    settings.store(Scope::Global, &Brightness(40)).unwrap();
    assert_eq!(settings.load::<Lighting>(Scope::Global).unwrap(), None);
    settings
        .store(
            Scope::Global,
            &Lighting {
                brightness: 1,
                effect: 2,
            },
        )
        .unwrap();
    assert_eq!(settings.load::<Brightness>(Scope::Global).unwrap(), None);
}

#[test]
fn settings_count_their_loads_and_stores_into_the_diagnostics() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut opened, _) = open(nor, layout, false).unwrap();
    let settings = opened.with_diagnostics(&DIAGNOSTICS);
    settings.store(Scope::Global, &Brightness(40)).unwrap();
    settings.load::<Brightness>(Scope::Global).unwrap();
    settings
        .remove(RecordKey::of::<Brightness>(Scope::Global))
        .unwrap();
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
fn every_access_counts_once_into_the_diagnostics() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let mut seen = Vec::new();
    let mut note = |step: &'static str| {
        let storage = DIAGNOSTICS
            .report(Usb::default(), Hardware::default())
            .storage;
        seen.push((step, [storage.loads, storage.stores, storage.failures]));
    };
    let (nor, layout, _) = filled(20).unwrap();
    let (mut opened, _) = open(nor, layout, false).unwrap();
    let settings = opened.with_diagnostics(&DIAGNOSTICS);
    let key = RecordKey::of::<Lighting>(Scope::Profile(1));
    let absent = RecordKey::of::<Lighting>(Scope::Profile(9));
    settings
        .store_with(key, Lighting::VERSION, &|body| {
            let lighting = Lighting {
                brightness: 4,
                effect: 1,
            };
            postcard::to_slice(&lighting, body)
                .ok()
                .map(|body| body.len())
        })
        .unwrap();
    note("store");
    assert_eq!(
        settings.store_with(key, Lighting::VERSION, &|_| None),
        Err(Error::Unencodable)
    );
    note("refused store");
    assert!(settings.load_stored(key).unwrap().is_some());
    assert!(settings.load_stored(absent).unwrap().is_none());
    note("stored loads");
    assert_eq!(settings.load::<Lighting>(Scope::Profile(9)), Ok(None));
    note("record load");
    settings.visit(&mut |_, _| {}).unwrap();
    note("visit");
    settings.usage().unwrap();
    note("usage");
    settings.remove(absent).unwrap();
    settings.remove(key).unwrap();
    note("removes");
    assert_eq!(
        seen,
        [
            ("store", [0, 1, 0]),
            ("refused store", [0, 1, 1]),
            ("stored loads", [2, 1, 1]),
            ("record load", [3, 1, 1]),
            ("visit", [4, 1, 1]),
            ("usage", [5, 1, 1]),
            ("removes", [5, 3, 1]),
        ]
    );
}

#[test]
fn a_record_longer_than_the_largest_value_is_refused_and_keeps_the_stored_record() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let largest = text(LARGEST_BODY - 2, 0xA5);
    settings.store(Scope::Global, &largest).unwrap();
    assert_eq!(
        settings.store(Scope::Global, &text(LARGEST_BODY - 1, 0x5A)),
        Err(Error::Unencodable)
    );
    assert_eq!(settings.load::<Text>(Scope::Global).unwrap(), Some(largest));
    let (mut settings, _) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(
        settings.load::<Text>(Scope::Global).unwrap(),
        Some(text(LARGEST_BODY - 2, 0xA5))
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
    settings
        .store_with(key, Lighting::VERSION, &|body| {
            postcard::to_slice(&lighting, body)
                .ok()
                .map(|body| body.len())
        })
        .unwrap();
    assert_eq!(
        settings.load::<Lighting>(Scope::Profile(1)).unwrap(),
        Some(lighting)
    );
    assert_eq!(
        settings
            .load_stored(key)
            .unwrap()
            .map(|stored| stored.decode::<Lighting>()),
        Some(Some(lighting))
    );
    assert_eq!(
        settings
            .load_stored(RecordKey::of::<Lighting>(Scope::Global))
            .unwrap()
            .map(|stored| stored.version),
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
    settings.store(Scope::Global, &kept).unwrap();
    let key = RecordKey::of::<Lighting>(Scope::Global);
    assert_eq!(
        settings.store_with(key, Lighting::VERSION, &|_| None),
        Err(Error::Unencodable)
    );
    assert!(
        settings
            .store_with(key, Lighting::VERSION, &|body| Some(body.len() + 1))
            .is_err()
    );
    assert_eq!(
        settings.load::<Lighting>(Scope::Global).unwrap(),
        Some(kept)
    );
}

#[test]
fn a_key_that_was_never_stored_is_answered_without_reading_the_flash() {
    let (nor, layout, _) = filled(20).unwrap();
    let (probe, watch) = Probe::new(nor);
    let mut settings = Settings::new();
    let opened = settings.open(Partition::new(probe, layout.settings_region()), false);
    assert_eq!(opened, Ok(Opened::Kept));
    let absent = RecordKey::of::<Lighting>(Scope::Profile(9));
    let before = watch.reads();
    assert_eq!(settings.load::<Lighting>(Scope::Profile(9)), Ok(None));
    assert_eq!(settings.remove(absent), Ok(()));
    assert_eq!(watch.reads(), before);
    assert!(settings.load::<Blob>(Scope::Profile(3)).unwrap().is_some());
    assert!(watch.reads() > before);
}

#[test]
fn a_key_is_found_again_after_it_was_removed_and_stored_anew() {
    let (nor, layout, _) = filled(20).unwrap();
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let key = RecordKey::of::<Blob>(Scope::Profile(2));
    settings.remove(key).unwrap();
    assert_eq!(settings.load::<Blob>(Scope::Profile(2)).unwrap(), None);
    let lighting = Lighting {
        brightness: 3,
        effect: 4,
    };
    settings.store(Scope::Profile(2), &lighting).unwrap();
    let (mut settings, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
    assert_eq!(
        settings.load::<Lighting>(Scope::Profile(2)).unwrap(),
        Some(lighting)
    );
    assert_eq!(settings.load::<Blob>(Scope::Profile(2)).unwrap(), None);
    assert!(settings.load::<Blob>(Scope::Profile(1)).unwrap().is_some());
}

#[test]
fn a_record_of_the_largest_size_is_stored_and_read_back() {
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let key = RecordKey::of::<Lighting>(Scope::Profile(1));
    for len in [LARGEST_BODY - 2, LARGEST_BODY - 1, LARGEST_BODY] {
        settings
            .store_with(key, Lighting::VERSION, &|body| {
                body.get_mut(..len)?.fill(0xA5);
                Some(len)
            })
            .unwrap();
        assert_eq!(
            settings
                .load_stored(key)
                .map(|stored| stored.map(|stored| stored.body.len())),
            Ok(Some(len))
        );
    }
    assert!(
        settings
            .store_with(key, Lighting::VERSION, &|body| {
                body.get_mut(..=LARGEST_BODY)?.fill(0xA5);
                Some(LARGEST_BODY + 1)
            })
            .is_err()
    );
    assert_eq!(
        settings
            .load_stored(key)
            .map(|stored| stored.map(|stored| stored.body.len())),
        Ok(Some(LARGEST_BODY))
    );
    let (mut settings, opened) = open(close(settings).unwrap(), layout, false).unwrap();
    assert_eq!(opened, Opened::Kept);
    assert_eq!(
        settings
            .load_stored(key)
            .map(|stored| stored.map(|stored| stored.body.len())),
        Ok(Some(LARGEST_BODY))
    );
}
