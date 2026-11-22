use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    config::{Capabilities, Invalid, Validate},
    keys::{Action, Distance, Index, Layer},
};

pub const MAX_ENTRIES: usize = 40;
pub const STROKES: usize = 4;

pub type Keys = Vec<Key, MAX_ENTRIES>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Key {
    pub key: Index,
    pub layer: Layer,
    pub behaviour: Behaviour,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Behaviour {
    DynamicKeystroke(DynamicKeystroke),
    ModTap(ModTap),
    Toggle(Action),
    DepthPriority(DepthPriority),
    Socd(Socd),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct DynamicKeystroke {
    pub strokes: [Stroke; STROKES],
    pub bottom_out: Option<Distance>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Stroke {
    pub action: Action,
    pub on_press: StrokeEvent,
    pub on_bottom_out: StrokeEvent,
    pub on_lift: StrokeEvent,
    pub on_release: StrokeEvent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum StrokeEvent {
    #[default]
    Nothing,
    Press,
    Release,
    Tap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ModTap {
    pub tap: Action,
    pub hold: Action,
    pub hold_time_ms: u16,
    pub interrupt: Interrupt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Interrupt {
    #[default]
    Auto,
    Ignore,
    Hold,
    Tap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct DepthPriority {
    pub other: Index,
    pub both_bottomed_out: Resolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Socd {
    pub other: Index,
    pub resolution: Resolution,
    pub both_when_bottomed_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Resolution {
    Both,
    Neither,
    First,
    Second,
    LastPressed,
}

impl ModTap {
    pub const DEFAULT_HOLD_TIME_MS: u16 = 200;
}

impl Key {
    pub fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key(self.key)?;
        match &self.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => {
                keystroke.bottom_out.map_or(Ok(()), Distance::validate)?;
                keystroke
                    .strokes
                    .iter()
                    .try_for_each(|stroke| stroke.action.validate(capabilities))
            }
            Behaviour::ModTap(mod_tap) => {
                if mod_tap.hold_time_ms == 0 {
                    return Err(Invalid::ZeroHoldTime);
                }
                mod_tap.tap.validate(capabilities)?;
                mod_tap.hold.validate(capabilities)
            }
            Behaviour::Toggle(action) => action.validate(capabilities),
            Behaviour::DepthPriority(DepthPriority { other, .. })
            | Behaviour::Socd(Socd { other, .. }) => {
                capabilities.check_key(*other)?;
                if *other == self.key {
                    Err(Invalid::KeyPairedWithItself(other.get()))
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Validate for Keys {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.iter().try_for_each(|key| key.validate(capabilities))
    }
}
