use heapless::String;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    keys::Layer,
    lighting::Rgb,
    wire::{Decode, Encode},
};

pub const NAME_LEN: usize = 32;
pub const MAX_LINKED: u8 = 10;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Schema,
    Encode,
    Decode,
)]
pub struct Slot(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Summary {
    pub count: u8,
    pub active: Slot,
    pub linked: u8,
    pub selected: Option<Slot>,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Namespace {
    #[default]
    Onboard,
    Linked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Selection {
    pub profile: Option<Slot>,
    pub activate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Info {
    pub name: String<NAME_LEN>,
    pub color: Rgb,
    pub icon: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct LayerRef {
    pub profile: Slot,
    pub layer: Layer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Change<T> {
    pub profile: Slot,
    pub value: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct LayerChange<T> {
    pub profile: Slot,
    pub layer: Layer,
    pub value: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Swap {
    pub first: Slot,
    pub second: Slot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Move {
    pub from: Slot,
    pub to: Slot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct State {
    pub active: Slot,
    pub host_control: bool,
    pub idle_off: bool,
}

impl Slot {
    const LINKED: u8 = 0x80;

    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn linked(index: u8) -> Self {
        Self(Self::LINKED | index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn is_linked(self) -> bool {
        self.0 & Self::LINKED != 0
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.0 & !Self::LINKED
    }

    pub fn check_onboard(self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if self.is_linked() {
            Err(Invalid::ProfileLinked(self.0))
        } else {
            self.validate(capabilities)
        }
    }

    pub fn check_linked(self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if self.is_linked() {
            self.validate(capabilities)
        } else {
            Err(Invalid::ProfileNotLinked(self.0))
        }
    }
}

impl Validate for Slot {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        let slots = if self.is_linked() {
            capabilities.linked_profiles
        } else {
            capabilities.profiles
        };
        if self.index() < slots {
            Ok(())
        } else {
            Err(Invalid::ProfileOutsideRange(self.0))
        }
    }
}

impl Validate for Selection {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.profile
            .map_or(Ok(()), |profile| profile.check_linked(capabilities))
    }
}

impl Validate for LayerRef {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.profile.validate(capabilities)
    }
}

impl Validate for Info {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        Ok(())
    }
}

impl Validate for Swap {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.first.check_onboard(capabilities)?;
        self.second.check_onboard(capabilities)
    }
}

impl Validate for Move {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.from.check_onboard(capabilities)?;
        self.to.check_onboard(capabilities)
    }
}

impl<T: Validate> Validate for Change<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.profile.validate(capabilities)?;
        self.value.validate(capabilities)
    }
}

impl<T: Validate> Validate for LayerChange<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.profile.validate(capabilities)?;
        self.value.validate(capabilities)
    }
}
