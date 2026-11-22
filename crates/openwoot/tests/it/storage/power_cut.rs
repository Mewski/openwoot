use embassy_futures::block_on;
use openwoot::storage::{Opened, Scope};

use super::{
    Blob, PROFILES, SETTINGS_SECTORS, blob, close, device_info_untouched, filled, open, profile_of,
};

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
