use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use super::Invalid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, thiserror::Error)]
pub enum Rejection {
    #[error(transparent)]
    Invalid(#[from] Invalid),
    #[error("the keyboard does not support the request")]
    Unsupported,
    #[error("profile {0} does not exist")]
    ProfileMissing(u8),
    #[error("the keyboard holds no further profile")]
    ProfileLimitReached,
    #[error("profile {0} cannot be deleted")]
    ProfileProtected(u8),
    #[error("the keyboard is locked")]
    Locked,
    #[error("could not access the settings storage")]
    StorageInaccessible,
}
