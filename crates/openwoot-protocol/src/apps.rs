use heapless::{String, Vec};
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    profile::Slot,
};

pub const MAX_LINKS: u8 = 16;
pub const MAX_FILTERS: usize = 4;
pub const TEXT_LEN: usize = 260;

pub type Text = String<TEXT_LEN>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Link {
    pub name: Text,
    pub app: App,
}

#[expect(
    clippy::large_enum_variant,
    reason = "the protocol has no heap, so a custom app is held in place as in the stock message"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum App {
    Custom(Custom),
    Steam(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Custom {
    pub matching: Matching,
    pub filters: Vec<Filter, MAX_FILTERS>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Matching {
    #[default]
    Any,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Filter {
    ProcessName(Text),
    ProcessDirectory(Text),
    WindowTitle(Text),
    ProcessPath(Text),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct LinkRef {
    pub profile: Slot,
    pub index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct LinkChange {
    pub profile: Slot,
    pub index: u8,
    pub link: Link,
}

impl Validate for Link {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        match &self.app {
            App::Custom(custom) if custom.filters.is_empty() => Err(Invalid::AppWithoutFilter),
            App::Custom(_) | App::Steam(_) => Ok(()),
        }
    }
}

impl Validate for LinkRef {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.profile.check_linked(capabilities)?;
        if self.index < MAX_LINKS {
            Ok(())
        } else {
            Err(Invalid::AppLinkOutsideRange(self.index))
        }
    }
}

impl Validate for LinkChange {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        LinkRef {
            profile: self.profile,
            index: self.index,
        }
        .validate(capabilities)?;
        self.link.validate(capabilities)
    }
}
