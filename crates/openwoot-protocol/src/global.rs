use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    lighting::Idle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub idle: Option<Idle>,
    pub num_lock_inverted: bool,
    pub release_rebound_filter: bool,
}

impl Validate for Settings {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        Ok(())
    }
}
