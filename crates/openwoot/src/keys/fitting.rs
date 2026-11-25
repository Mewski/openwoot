use heapless::Vec;
use openwoot_protocol::{
    configuration::{Invalid, MAX_KEYS},
    switches::{self, MAX_MODELS, MAX_TUNED},
};

use crate::analog::{Switch, SwitchModel};

const _: () = assert!(
    SwitchModel::COUNT as usize <= MAX_MODELS,
    "the switch models do not fit the model list of the protocol"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FittedSwitches {
    settings: switches::Settings,
    tuned: Vec<Switch, MAX_TUNED>,
}

impl FittedSwitches {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            settings: switches::Settings {
                default: None,
                keys: Vec::new(),
            },
            tuned: Vec::new(),
        }
    }

    #[must_use]
    pub const fn settings(&self) -> &switches::Settings {
        &self.settings
    }

    #[must_use]
    pub fn switch(&self, index: usize, factory: SwitchModel) -> Switch {
        let Some(chosen) = self.settings.of_key(index) else {
            return Switch::untuned(factory);
        };
        let Some(kind) = SwitchModel::new(chosen.model) else {
            return Switch::untuned(factory);
        };
        if !chosen.is_tuned() {
            return Switch::untuned(kind);
        }
        self.tuned
            .iter()
            .find(|tuned| {
                tuned.kind() == kind && tuned.travel_offset() == Some(chosen.travel_offset)
            })
            .copied()
            .unwrap_or(Switch::untuned(kind))
    }

    pub(super) fn replace(
        &mut self,
        settings: &switches::Settings,
        tuned: Vec<Switch, MAX_TUNED>,
    ) -> bool {
        let changed =
            (0..MAX_KEYS).any(|index| self.settings.of_key(index) != settings.of_key(index));
        self.settings.clone_from(settings);
        self.tuned = tuned;
        changed
    }
}

impl Default for FittedSwitches {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn switch_models() -> switches::Models {
    let mut models = switches::Models::new();
    for kind in (0..SwitchModel::COUNT).filter_map(SwitchModel::new) {
        let model = switches::Model {
            travel: kind.travel(),
            group: kind.group(),
        };
        if models.push(model).is_err() {
            break;
        }
    }
    models
}

pub(super) fn tune(settings: &switches::Settings) -> Result<Vec<Switch, MAX_TUNED>, Invalid> {
    settings.check_models(&switch_models())?;
    let mut tuned = Vec::<Switch, MAX_TUNED>::new();
    for chosen in settings.switches().filter(|chosen| chosen.is_tuned()) {
        let Some(kind) = SwitchModel::new(chosen.model) else {
            continue;
        };
        let offset = Some(chosen.travel_offset);
        if tuned
            .iter()
            .any(|known| known.kind() == kind && known.travel_offset() == offset)
        {
            continue;
        }
        tuned
            .push(Switch::new(kind, offset))
            .map_err(|_| Invalid::TunedSwitchesExceedLimit)?;
    }
    Ok(tuned)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::switches::Switch as Chosen;

    use super::*;

    const fn chosen(model: u8, travel_offset: u8) -> Chosen {
        Chosen {
            model,
            travel_offset,
        }
    }

    #[test]
    fn the_model_list_carries_the_travel_and_group_of_every_known_model() {
        let models = switch_models();
        assert_eq!(models.len(), usize::from(SwitchModel::COUNT));
        for (id, model) in (0..).zip(&models) {
            let kind = SwitchModel::new(id).unwrap();
            assert_eq!(model.travel, kind.travel());
            assert_eq!(model.group, kind.group());
        }
    }

    #[test]
    fn a_key_uses_its_own_switch_then_the_default_then_the_factory_model() {
        let factory = SwitchModel::new(1).unwrap();
        let settings = switches::Settings {
            default: Some(chosen(2, 0)),
            keys: Vec::from_iter([Some(chosen(3, 10)), None]),
        };
        let mut fitted = FittedSwitches::new();
        assert_eq!(fitted.switch(0, factory), Switch::untuned(factory));
        let tuned = tune(&settings).unwrap();
        assert!(fitted.replace(&settings, tuned));
        let third = SwitchModel::new(3).unwrap();
        assert_eq!(fitted.switch(0, factory), Switch::new(third, Some(10)));
        let second = SwitchModel::new(2).unwrap();
        assert_eq!(fitted.switch(1, factory), Switch::untuned(second));
        assert_eq!(fitted.switch(MAX_KEYS, factory), Switch::untuned(second));
    }

    #[test]
    fn replacing_switches_with_the_same_switch_per_key_is_no_change() {
        let mut fitted = FittedSwitches::new();
        let factory = switches::Settings {
            default: None,
            keys: Vec::from_iter([None, None]),
        };
        assert!(!fitted.replace(&factory, Vec::new()));
        assert_eq!(fitted.settings(), &factory);
        let spelled_out = switches::Settings {
            default: Some(chosen(4, 2)),
            keys: Vec::new(),
        };
        let per_key = switches::Settings {
            default: None,
            keys: core::iter::repeat_n(Some(chosen(4, 2)), MAX_KEYS).collect(),
        };
        assert!(fitted.replace(&spelled_out, tune(&spelled_out).unwrap()));
        assert!(!fitted.replace(&per_key, tune(&per_key).unwrap()));
    }

    #[test]
    fn each_tuned_pair_of_model_and_offset_is_fitted_once() {
        let settings = switches::Settings {
            default: Some(chosen(0, 4)),
            keys: Vec::from_iter([
                Some(chosen(0, 4)),
                Some(chosen(0, 0)),
                Some(chosen(0, 5)),
                Some(chosen(1, 4)),
            ]),
        };
        let tuned = tune(&settings).unwrap();
        let pairs: Vec<_, MAX_TUNED> = tuned
            .iter()
            .map(|switch| (switch.kind().id(), switch.travel_offset()))
            .collect();
        assert_eq!(pairs, [(0, Some(4)), (0, Some(5)), (1, Some(4))]);
    }

    #[test]
    fn switches_the_keyboard_cannot_fit_are_refused() {
        let beyond = switches::Settings {
            default: Some(chosen(0, 80)),
            keys: Vec::new(),
        };
        assert_eq!(
            tune(&beyond),
            Err(Invalid::TravelOffsetReachesTravel {
                model: 0,
                offset: 80,
                travel: 80
            })
        );
        let unknown = switches::Settings {
            default: None,
            keys: Vec::from_iter([Some(chosen(SwitchModel::COUNT, 0))]),
        };
        assert_eq!(
            tune(&unknown),
            Err(Invalid::SwitchModelUnknown(SwitchModel::COUNT))
        );
        let many = switches::Settings {
            default: None,
            keys: (1..=31).map(|offset| Some(chosen(0, offset))).collect(),
        };
        assert_eq!(tune(&many), Err(Invalid::TunedSwitchesExceedLimit));
    }

    #[test]
    fn thirty_distinct_tuned_switches_are_fitted_and_a_thirty_first_is_refused() {
        let settings = switches::Settings {
            default: Some(chosen(1, 30)),
            keys: (1..=30).map(|offset| Some(chosen(0, offset))).collect(),
        };
        assert_eq!(
            tune(&settings),
            Err(Invalid::TunedSwitchesExceedLimit),
            "the default adds a 31st pair"
        );
        let settings = switches::Settings {
            default: Some(chosen(0, 30)),
            ..settings
        };
        let tuned = tune(&settings).unwrap();
        assert_eq!(tuned.len(), 30);
        let mut fitted = FittedSwitches::new();
        assert!(fitted.replace(&settings, tuned));
        let first = SwitchModel::new(0).unwrap();
        for (index, offset) in (1..=30).enumerate() {
            assert_eq!(
                fitted.switch(index, first),
                Switch::new(first, Some(offset))
            );
        }
    }
}
