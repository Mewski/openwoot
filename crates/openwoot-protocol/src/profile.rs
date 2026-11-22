use heapless::String;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    config::{Capabilities, Invalid, Validate},
    keys::Layer,
    lighting::Rgb,
};

pub const NAME_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Schema)]
pub struct Slot(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Summary {
    pub count: u8,
    pub active: Slot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Info {
    pub name: String<NAME_LEN>,
    pub color: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct LayerRef {
    pub profile: Slot,
    pub layer: Layer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Change<T> {
    pub profile: Slot,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct LayerChange<T> {
    pub profile: Slot,
    pub layer: Layer,
    pub value: T,
}

impl Slot {
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Validate for Slot {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_profile(*self)
    }
}

impl Validate for LayerRef {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_profile(self.profile)
    }
}

impl Validate for Info {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        Ok(())
    }
}

impl<T: Validate> Validate for Change<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_profile(self.profile)?;
        self.value.validate(capabilities)
    }
}

impl<T: Validate> Validate for LayerChange<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_profile(self.profile)?;
        self.value.validate(capabilities)
    }
}
