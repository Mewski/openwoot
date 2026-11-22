use openwoot_protocol::lighting::Settings;

use super::Colors;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub settings: Settings,
    pub colors: Colors,
}
