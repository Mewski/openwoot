use std::collections::BTreeSet;

use openwoot_protocol::{
    configuration::Capabilities,
    switches::{self, Model, Switch},
};

pub(super) const MILLIMETRES_PER_STEP: f32 = 0.05;
const FACTORY_MODEL: u8 = 0;
const EDITION_FACTORY_MODEL: u8 = 1;

pub(super) const fn factory(capabilities: &Capabilities) -> Switch {
    Switch {
        model: if capabilities.edition.is_some() {
            EDITION_FACTORY_MODEL
        } else {
            FACTORY_MODEL
        },
        travel_offset: 0,
    }
}

pub(super) fn of_key(settings: &switches::Settings, factory: Switch, key: usize) -> Switch {
    settings.of_key(key).unwrap_or(factory)
}

pub(super) fn assign(
    settings: &mut switches::Settings,
    keys: &[usize],
    factory: Switch,
    model: u8,
    models: &[Model],
) {
    let travel = models
        .get(usize::from(model))
        .map_or(0, |entry| entry.travel);
    for key in keys {
        let offset = of_key(settings, factory, *key).travel_offset;
        set(
            settings,
            *key,
            factory,
            Switch {
                model,
                travel_offset: if offset < travel { offset } else { 0 },
            },
        );
    }
}

pub(super) fn set_spacers(
    settings: &mut switches::Settings,
    keys: &[usize],
    factory: Switch,
    offset: impl Fn(Switch) -> u8,
) {
    for key in keys {
        let current = of_key(settings, factory, *key);
        set(
            settings,
            *key,
            factory,
            Switch {
                model: current.model,
                travel_offset: offset(current),
            },
        );
    }
}

pub(super) fn fold(settings: &mut switches::Settings, present: &[usize], factory: Switch) {
    let mut effective = present.iter().map(|key| of_key(settings, factory, *key));
    let Some(first) = effective.next() else {
        return;
    };
    if effective.all(|switch| switch == first) {
        settings.default = (first != factory).then_some(first);
        for slot in &mut settings.keys {
            *slot = None;
        }
    }
}

pub(super) fn tuned_pairs(settings: &switches::Settings) -> usize {
    settings
        .switches()
        .filter(|switch| switch.is_tuned())
        .map(|switch| (switch.model, switch.travel_offset))
        .collect::<BTreeSet<_>>()
        .len()
}

pub(super) fn millimetres(steps: u8) -> f32 {
    f32::from(steps) * MILLIMETRES_PER_STEP
}

pub(super) fn steps_of(millimetres: f32) -> u8 {
    let steps = (millimetres / MILLIMETRES_PER_STEP)
        .round()
        .clamp(0.0, f32::from(u8::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=255"
    )]
    let steps = steps as u8;
    steps
}

fn set(settings: &mut switches::Settings, key: usize, factory: Switch, switch: Switch) {
    let inherited = settings.default.unwrap_or(factory);
    if let Some(slot) = settings.keys.get_mut(key) {
        *slot = (switch != inherited).then_some(switch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn switch(model: u8, travel_offset: u8) -> Switch {
        Switch {
            model,
            travel_offset,
        }
    }

    fn capabilities(edition: Option<u8>) -> Capabilities {
        Capabilities {
            key_rows: 1,
            key_columns: 4,
            profiles: 1,
            linked_profiles: 0,
            layouts: std::iter::empty().collect(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 49,
            edition,
        }
    }

    fn settings(keys: [Option<Switch>; 4]) -> switches::Settings {
        switches::Settings {
            default: None,
            keys: keys.into_iter().collect(),
        }
    }

    const MODELS: [Model; 3] = [
        Model {
            travel: 80,
            group: 0,
        },
        Model {
            travel: 80,
            group: 0,
        },
        Model {
            travel: 66,
            group: 0,
        },
    ];

    #[test]
    fn spacer_sizes_round_to_steps_of_five_hundredths_of_a_millimetre() {
        assert_eq!(steps_of(0.5), 10);
        assert_eq!(steps_of(0.524), 10);
        assert_eq!(steps_of(0.53), 11);
        assert_eq!(steps_of(-1.0), 0);
        assert_eq!(steps_of(100.0), u8::MAX);
        assert!((millimetres(10) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn keys_without_a_switch_use_the_factory_switch_of_the_unit() {
        assert_eq!(factory(&capabilities(None)), switch(FACTORY_MODEL, 0));
        assert_eq!(
            factory(&capabilities(Some(9))),
            switch(EDITION_FACTORY_MODEL, 0)
        );
    }

    #[test]
    fn a_new_switch_keeps_a_spacer_only_while_it_is_shorter_than_its_travel() {
        let mut keys = settings([Some(switch(0, 70)), Some(switch(0, 10)), None, None]);
        assign(&mut keys, &[0, 1], switch(0, 0), 2, &MODELS);
        assert_eq!(keys.keys[0], Some(switch(2, 0)));
        assert_eq!(keys.keys[1], Some(switch(2, 10)));
    }

    #[test]
    fn spacers_change_only_the_selected_keys() {
        let mut keys = settings([None, Some(switch(2, 0)), None, None]);
        set_spacers(&mut keys, &[1, 2], switch(0, 0), |_| 10);
        assert_eq!(keys.keys[1], Some(switch(2, 10)));
        assert_eq!(keys.keys[2], Some(switch(0, 10)));
        assert_eq!(keys.keys[0], None);
        set_spacers(&mut keys, &[2], switch(0, 0), |_| 0);
        assert_eq!(keys.keys[2], None);
    }

    #[test]
    fn a_switch_every_key_shares_becomes_the_default() {
        let mut keys = settings([None, None, None, None]);
        assign(&mut keys, &[0, 1, 2, 3], switch(0, 0), 2, &MODELS);
        fold(&mut keys, &[0, 1, 2, 3], switch(0, 0));
        assert_eq!(keys.default, Some(switch(2, 0)));
        assert!(keys.keys.iter().all(Option::is_none));
        assign(&mut keys, &[0, 1, 2, 3], switch(0, 0), 0, &MODELS);
        fold(&mut keys, &[0, 1, 2, 3], switch(0, 0));
        assert_eq!(keys.default, None);
        assert!(keys.keys.iter().all(Option::is_none));
    }

    #[test]
    fn keys_that_differ_keep_their_own_switch() {
        let mut keys = settings([Some(switch(2, 0)), None, None, None]);
        fold(&mut keys, &[0, 1, 2, 3], switch(0, 0));
        assert_eq!(keys.keys[0], Some(switch(2, 0)));
    }

    #[test]
    fn each_tuned_pair_of_switch_and_spacer_counts_once() {
        let keys = switches::Settings {
            default: Some(switch(0, 10)),
            keys: [
                Some(switch(0, 10)),
                Some(switch(2, 10)),
                None,
                Some(switch(2, 0)),
            ]
            .into_iter()
            .collect(),
        };
        assert_eq!(tuned_pairs(&keys), 2);
    }
}
