use openwoot::storage::{FormatReason, Opened, Partition, RecordKey, Scope, Settings};

use super::support::{
    Blob, PROFILES, SETTINGS_SECTORS, blob, chip, close, device_info_untouched, filled, open,
    profile_of,
};

const WIDE_SECTORS: u32 = 16;
const WIDE_ITEMS: u8 = 10;
const WIDE_UPDATES: u32 = 1500;
const FIRST_WIDE_ITEM: u8 = 0x40;

fn scrambled(state: u32) -> u32 {
    state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn wide_key(profile: u8, item: u8) -> RecordKey {
    RecordKey::new(Scope::Profile(profile), FIRST_WIDE_ITEM + item)
}

#[test]
fn a_power_cut_at_any_step_of_a_store_keeps_every_other_setting() {
    let (mut base, layout, mut expected) = filled(0).unwrap();
    for updates in 0..150 {
        let target = profile_of(updates);
        let new = blob(10_000 + updates);
        for cut in 0.. {
            let (mut settings, _) = open(base.cut_after(cut), layout, false).unwrap();
            let stored = settings.store(Scope::Profile(target), &new);
            let nor = close(settings).unwrap();
            if !nor.was_cut() {
                stored.unwrap();
                break;
            }
            let (mut settings, opened) = open(nor.revived(), layout, false).unwrap();
            assert_eq!(opened, Opened::Kept, "update {updates}, cut {cut}");
            for profile in 0..PROFILES {
                let loaded = settings.load::<Blob>(Scope::Profile(profile)).unwrap();
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
            settings.store(Scope::Profile(target), &new).unwrap();
            assert_eq!(
                settings.load::<Blob>(Scope::Profile(target)).unwrap(),
                Some(new)
            );
            assert!(device_info_untouched(&close(settings).unwrap()));
        }
        let (mut settings, _) = open(base, layout, false).unwrap();
        settings.store(Scope::Profile(target), &new).unwrap();
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
        let reset = settings.factory_reset();
        let nor = close(settings).unwrap();
        if !nor.was_cut() {
            reset.unwrap();
            break;
        }
        let (mut settings, _) = open(nor.revived(), layout, false).unwrap();
        settings.store(Scope::Profile(0), &blob(1)).unwrap();
        assert_eq!(
            settings.load::<Blob>(Scope::Profile(0)).unwrap(),
            Some(blob(1))
        );
        assert!(device_info_untouched(&close(settings).unwrap()));
    }
}

#[test]
fn a_power_cut_during_a_factory_reset_never_keeps_a_part_of_the_settings() {
    let (nor, layout) = chip(WIDE_SECTORS);
    let (mut settings, _) = open(nor, layout, false).unwrap();
    let mut state = 0x2545_F491;
    for update in 0..WIDE_UPDATES {
        state = scrambled(state);
        let [_, profile, item, _] = state.to_le_bytes();
        let (profile, item) = match u8::try_from(update) {
            Ok(first) if first < PROFILES * WIDE_ITEMS => (first / WIDE_ITEMS, first % WIDE_ITEMS),
            _ => (profile % PROFILES, item % WIDE_ITEMS),
        };
        settings
            .store_with(wide_key(profile, item), 1, &|body| {
                postcard::to_slice(&blob(update), body)
                    .ok()
                    .map(|body| body.len())
            })
            .unwrap();
    }
    let base = close(settings).unwrap();
    let every = usize::from(PROFILES) * usize::from(WIDE_ITEMS);
    for cut in 0.. {
        let (mut settings, _) = open(base.cut_after(cut), layout, false).unwrap();
        let reset = settings.factory_reset();
        let nor = close(settings).unwrap();
        if !nor.was_cut() {
            reset.unwrap();
            break;
        }
        let (mut settings, _) = open(nor.revived(), layout, false).unwrap();
        let mut kept = 0;
        for profile in 0..PROFILES {
            for item in 0..WIDE_ITEMS {
                let found = settings.load_stored(wide_key(profile, item));
                kept += usize::from(found.unwrap().is_some());
            }
        }
        assert!(kept == 0 || kept == every, "cut {cut}: {kept} of {every}");
        assert!(device_info_untouched(&close(settings).unwrap()));
    }
}

#[test]
fn a_power_cut_at_any_step_of_a_remove_keeps_every_other_setting() {
    for updates in [0, 23, 150] {
        let (base, layout, expected) = filled(updates).unwrap();
        let target = profile_of(updates);
        let key = RecordKey::of::<Blob>(Scope::Profile(target));
        for cut in 0.. {
            let (mut settings, _) = open(base.cut_after(cut), layout, false).unwrap();
            let removed = settings.remove(key);
            let nor = close(settings).unwrap();
            if !nor.was_cut() {
                removed.unwrap();
                break;
            }
            let (mut settings, opened) = open(nor.revived(), layout, false).unwrap();
            assert_eq!(opened, Opened::Kept, "updates {updates}, cut {cut}");
            for profile in 0..PROFILES {
                let loaded = settings.load::<Blob>(Scope::Profile(profile)).unwrap();
                let old = expected[usize::from(profile)];
                if profile == target {
                    assert!(
                        loaded.is_none() || loaded == Some(old),
                        "updates {updates}, cut {cut}"
                    );
                } else {
                    assert_eq!(
                        loaded,
                        Some(old),
                        "updates {updates}, cut {cut}, profile {profile}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_power_cut_while_foreign_content_is_replaced_leaves_a_working_store() {
    let (mut foreign, layout) = chip(SETTINGS_SECTORS);
    let mut state = 0x1234_5678;
    for sector in 0..SETTINGS_SECTORS {
        for byte in foreign.sector_mut(sector) {
            state = scrambled(state);
            [*byte, ..] = state.to_be_bytes();
        }
    }
    for cut in 0.. {
        let mut settings = Settings::new();
        let opened = settings.open(
            Partition::new(foreign.cut_after(cut), layout.settings_region()),
            false,
        );
        let nor = close(settings).unwrap();
        if !nor.was_cut() {
            assert_eq!(opened, Ok(Opened::Formatted(FormatReason::Unrecognised)));
            break;
        }
        let (mut settings, opened) = open(nor.revived(), layout, false).unwrap();
        assert_eq!(
            opened,
            Opened::Formatted(FormatReason::Unrecognised),
            "cut {cut}"
        );
        settings.store(Scope::Profile(0), &blob(1)).unwrap();
        assert_eq!(
            settings.load::<Blob>(Scope::Profile(0)).unwrap(),
            Some(blob(1))
        );
        assert!(device_info_untouched(&close(settings).unwrap()));
    }
}
