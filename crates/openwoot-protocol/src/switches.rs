use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::configuration::{Capabilities, Invalid, MAX_KEYS, Validate};

pub const MAX_MODELS: usize = 64;

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

impl Validate for Switch {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if self.model < capabilities.switch_models {
            Ok(())
        } else {
            Err(Invalid::SwitchModel(self.model))
        }
    }
}

impl Validate for Settings {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.keys.len())?;
        self.default
            .iter()
            .chain(self.keys.iter().flatten())
            .try_for_each(|switch| switch.validate(capabilities))
    }
}
