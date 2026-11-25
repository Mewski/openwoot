use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::configuration::{Capabilities, Invalid, MAX_KEYS, Validate};

pub const MAX_MODELS: usize = 64;
pub const MODEL_COUNT: u8 = 49;
pub const MAX_TUNED: usize = 30;

pub type Models = Vec<Model, MAX_MODELS>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Switch {
    pub model: u8,
    pub travel_offset: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub default: Option<Switch>,
    pub keys: Vec<Option<Switch>, MAX_KEYS>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Model {
    pub travel: u8,
    pub group: u8,
}

impl Switch {
    #[must_use]
    pub const fn is_tuned(&self) -> bool {
        self.travel_offset != 0
    }

    pub fn check_travel(&self, models: &[Model]) -> Result<(), Invalid> {
        let model = models
            .get(usize::from(self.model))
            .ok_or(Invalid::SwitchModelUnknown(self.model))?;
        if self.travel_offset < model.travel {
            Ok(())
        } else {
            Err(Invalid::TravelOffsetReachesTravel {
                model: self.model,
                offset: self.travel_offset,
                travel: model.travel,
            })
        }
    }
}

impl Settings {
    #[must_use]
    pub fn of_key(&self, index: usize) -> Option<Switch> {
        self.keys.get(index).copied().flatten().or(self.default)
    }

    pub fn switches(&self) -> impl Iterator<Item = &Switch> {
        self.default.iter().chain(self.keys.iter().flatten())
    }

    pub fn check_models(&self, models: &[Model]) -> Result<(), Invalid> {
        let mut tuned = Vec::<Switch, MAX_TUNED>::new();
        for switch in self.switches() {
            switch.check_travel(models)?;
            if switch.is_tuned() && !tuned.contains(switch) {
                tuned
                    .push(*switch)
                    .map_err(|_| Invalid::TunedSwitchesExceedLimit)?;
            }
        }
        Ok(())
    }
}

impl Validate for Switch {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if self.model < capabilities.switch_models {
            Ok(())
        } else {
            Err(Invalid::SwitchModelUnknown(self.model))
        }
    }
}

impl Validate for Settings {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.keys.len())?;
        self.switches()
            .try_for_each(|switch| switch.validate(capabilities))
    }
}
