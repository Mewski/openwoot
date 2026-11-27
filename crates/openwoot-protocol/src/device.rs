use heapless::String;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    wire::{Decode, Encode},
};

pub const NAME_LEN: usize = 40;

pub type Name = String<NAME_LEN>;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub struct Settings {
    pub mode: Mode,
    pub keyboard: KeyboardFormat,
    pub speed: Speed,
    pub gamepad_polling: PollingRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct SettingsState {
    pub active: Settings,
    pub chosen: Settings,
    pub negotiated: Option<Speed>,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Mode {
    Xinput,
    Gamepad,
    #[default]
    NoGamepad,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum KeyboardFormat {
    #[default]
    NKeyRollover,
    Boot,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Speed {
    Full,
    #[default]
    High,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum PollingRate {
    Every4Milliseconds,
    #[default]
    Every1Millisecond,
    Every125Microseconds,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Layout {
    #[default]
    Ansi,
    Iso,
    Jis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct LayoutState {
    pub active: Layout,
    pub chosen: Layout,
}

impl Validate for Settings {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        Ok(())
    }
}

impl Validate for Layout {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if capabilities.layouts.contains(self) {
            Ok(())
        } else {
            Err(Invalid::LayoutMissing(*self))
        }
    }
}
