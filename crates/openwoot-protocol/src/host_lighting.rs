use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    keys::Position,
    lighting::Rgb,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct KeyColor {
    pub key: Position,
    pub color: Rgb,
}

impl Validate for KeyColor {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.key.validate(capabilities)
    }
}
