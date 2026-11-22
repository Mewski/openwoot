use embassy_futures::block_on;
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    storage::{Key, Record, Scope},
};
use openwoot_protocol::diagnostics::{Storage, Usb};
use serde::{Deserialize, Serialize};

use super::support::{Blob, Brightness, PROFILES, SETTINGS_SECTORS, chip, filled, open};

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
fn settings_count_their_loads_and_stores_into_the_diagnostics() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let (nor, layout) = chip(SETTINGS_SECTORS);
    let (mut opened, _) = open(nor, layout, false).unwrap();
    let settings = opened.with_diagnostics(&DIAGNOSTICS);
    block_on(settings.store(Scope::Global, &Brightness(40))).unwrap();
    block_on(settings.load::<Brightness>(Scope::Global)).unwrap();
    block_on(settings.remove(Key::of::<Brightness>(Scope::Global))).unwrap();
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
    let key = Key::of::<Lighting>(Scope::Profile(1));
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
            settings.load_with(Key::of::<Lighting>(Scope::Global), |stored| {
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
    let key = Key::of::<Lighting>(Scope::Global);
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
