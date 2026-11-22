use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    device::Layout,
    keys::{Index, Position},
    lighting::Idle,
    profile::Slot,
};

pub const MAX_KEYS: usize = 128;
pub const LAYOUTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Capabilities {
    pub key_rows: u8,
    pub key_columns: u8,
    pub profiles: u8,
    pub layouts: Vec<Layout, LAYOUTS>,
    pub lightbar_leds: u8,
    pub host_signals: bool,
    pub switch_models: u8,
    pub edition: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct GlobalSettings {
    pub idle: Option<Idle>,
    pub num_lock_inverted: bool,
    pub release_rebound_filter: bool,
}

pub trait Validate {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, thiserror::Error)]
pub enum Invalid {
    #[error("expected {expected} keys, found {found}")]
    KeyCount { expected: u16, found: u16 },
    #[error("key {0} lies outside the key matrix")]
    KeyOutsideMatrix(u8),
    #[error("row {row} column {column} lies outside the key matrix")]
    PositionOutsideMatrix { row: u8, column: u8 },
    #[error("key {0} is paired with itself")]
    KeyPairedWithItself(u8),
    #[error("keyboard usage `{0:#04x}` has no place in the keyboard report")]
    KeyboardUsage(u8),
    #[error("consumer usage `{0:#06x}` lies outside `0x0001`..`0x02a2`")]
    ConsumerUsage(u16),
    #[error("profile {0} lies outside the profiles of the keyboard")]
    ProfileOutsideRange(u8),
    #[error("the base layer cannot be a momentary layer")]
    MomentaryBaseLayer,
    #[error("a distance of zero")]
    ZeroDistance,
    #[error("a hold time of zero")]
    ZeroHoldTime,
    #[error("the response curve does not rise from the dead zone to the saturation point")]
    CurveNotRising,
    #[error("{0} % is more than 100 %")]
    Percentage(u8),
    #[error("{value} lies outside {min}..={max}")]
    Rate { value: u8, min: u8, max: u8 },
    #[error("light bar positions {first}..={last} lie outside the {leds} LEDs")]
    LightbarRange { first: u8, last: u8, leds: u8 },
    #[error("{count} colours do not fit the {leds} light bar LEDs")]
    LightbarColors { count: u8, leds: u8 },
    #[error("a period of zero")]
    ZeroPeriod,
    #[error("a meter maximum that is not a positive number")]
    MeterMaximum,
    #[error("a profile indicator without segments")]
    ZeroSegments,
    #[error("the keyboard has no light bar")]
    NoLightbar,
    #[error("the keyboard takes no host signals")]
    NoHostSignals,
    #[error("switch model {0} is unknown to the keyboard")]
    SwitchModel(u8),
    #[error("layout `{0:?}` does not exist on the keyboard")]
    Layout(Layout),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, thiserror::Error)]
pub enum Rejection {
    #[error(transparent)]
    Invalid(#[from] Invalid),
    #[error("the keyboard does not support the request")]
    Unsupported,
    #[error("profile {0} does not exist")]
    ProfileMissing(u8),
    #[error("the keyboard holds no further profile")]
    ProfileLimit,
    #[error("profile {0} cannot be deleted")]
    ProfileProtected(u8),
    #[error("the keyboard is locked")]
    Locked,
    #[error("could not access the settings storage")]
    Storage,
}

impl Capabilities {
    #[must_use]
    pub fn key_count(&self) -> usize {
        usize::from(self.key_rows) * usize::from(self.key_columns)
    }

    pub fn check_key(&self, key: Index) -> Result<(), Invalid> {
        if usize::from(key.get()) < self.key_count() {
            Ok(())
        } else {
            Err(Invalid::KeyOutsideMatrix(key.get()))
        }
    }

    pub fn check_position(&self, position: Position) -> Result<(), Invalid> {
        if position.row < self.key_rows && position.column < self.key_columns {
            Ok(())
        } else {
            Err(Invalid::PositionOutsideMatrix {
                row: position.row,
                column: position.column,
            })
        }
    }

    pub fn check_key_count(&self, found: usize) -> Result<(), Invalid> {
        let expected = self.key_count();
        if found == expected {
            Ok(())
        } else {
            Err(Invalid::KeyCount {
                expected: u16::try_from(expected).unwrap_or(u16::MAX),
                found: u16::try_from(found).unwrap_or(u16::MAX),
            })
        }
    }

    pub fn check_profile(&self, profile: Slot) -> Result<(), Invalid> {
        if profile.get() < self.profiles {
            Ok(())
        } else {
            Err(Invalid::ProfileOutsideRange(profile.get()))
        }
    }
}

impl Validate for GlobalSettings {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        Ok(())
    }
}

impl<T: Validate> Validate for Option<T> {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.as_ref()
            .map_or(Ok(()), |value| value.validate(capabilities))
    }
}
