use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    keys::Position,
    lightbar,
    lighting::{KeyColors, Rgb},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct KeyColor {
    pub key: Position,
    pub color: Rgb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Frame {
    pub keys: KeyColors,
    pub lightbar: lightbar::Colors,
}

impl Validate for KeyColor {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.key.validate(capabilities)
    }
}

impl Validate for lightbar::Colors {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if capabilities.lightbar_leds == 0 {
            return Err(Invalid::NoLightbar);
        }
        if self.len() > usize::from(capabilities.lightbar_leds) {
            return Err(Invalid::LightbarColorsExceedLeds {
                count: u8::try_from(self.len()).unwrap_or(u8::MAX),
                leds: capabilities.lightbar_leds,
            });
        }
        Ok(())
    }
}
