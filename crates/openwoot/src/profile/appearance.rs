use openwoot_protocol::{
    heapless::Vec,
    lightbar,
    lighting::{Color, Indicators, Settings},
};

use super::Colors;
use crate::{layout::LAYERS, lighting::lightbar_profile};

#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    pub settings: Settings,
    pub colors: Colors,
    pub lightbar: lightbar::Settings,
}

impl Appearance {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            settings: Settings {
                brightness: 0,
                indicators: Indicators {
                    caps_lock: Color::TRANSPARENT,
                    scroll_lock: Color::TRANSPARENT,
                    num_lock: Color::TRANSPARENT,
                    layer_lock: Color::TRANSPARENT,
                },
                effects: Vec::new(),
            },
            colors: Colors([const { None }; LAYERS]),
            lightbar: lightbar_profile(None),
        }
    }
}
