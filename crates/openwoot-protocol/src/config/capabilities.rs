use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::Invalid;
use crate::{
    device::Layout,
    keys::{Index, Position},
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
