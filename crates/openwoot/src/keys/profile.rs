use heapless::Vec;
use openwoot_protocol::{
    advanced,
    gamepad::{self, CurvePoint, Normalization, StickBias},
    keys::{self, Keymap, Overrides},
    wire::{Decode, Encode},
};

use super::default_settings;
use crate::{layout::LAYERS, storage::Record};

const IDENTITY_CURVE: [CurvePoint; gamepad::CURVE_POINTS] = [
    CurvePoint {
        travel: 0,
        output: 0,
    },
    CurvePoint {
        travel: 85,
        output: 85,
    },
    CurvePoint {
        travel: 170,
        output: 170,
    },
    CurvePoint {
        travel: 255,
        output: 255,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts(u8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub settings: keys::Settings,
    pub keymaps: Keymaps,
    pub overrides: Overrides,
    pub advanced: advanced::Keys,
    pub gamepad_mapping: gamepad::Mapping,
    pub gamepad_settings: gamepad::Settings,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Keymaps(pub [Keymap; LAYERS]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct ActiveProfile(pub u8);

impl Parts {
    pub const ACTIVATED: Self = Self(1 << 6);
    pub const ADVANCED: Self = Self(1 << 3);
    pub const ALL: Self = Self(0x3F);
    pub const GAMEPAD_MAPPING: Self = Self(1 << 4);
    pub const GAMEPAD_SETTINGS: Self = Self(1 << 5);
    pub const KEYMAPS: Self = Self(1 << 1);
    pub const NONE: Self = Self(0);
    pub const OVERRIDES: Self = Self(1 << 2);
    pub const PROFILES: Self = Self(1 << 7);
    pub const SETTINGS: Self = Self(1 << 0);

    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Profile {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            settings: default_settings(),
            keymaps: Keymaps([const { Vec::new() }; LAYERS]),
            overrides: Vec::new(),
            advanced: Vec::new(),
            gamepad_mapping: gamepad::Mapping {
                bindings: Vec::new(),
                exclusive: false,
            },
            gamepad_settings: default_gamepad_settings(),
        }
    }
}

#[must_use]
pub const fn default_gamepad_settings() -> gamepad::Settings {
    gamepad::Settings {
        curve: IDENTITY_CURVE,
        normalization: Normalization::Circular,
        single_direction_axes: false,
        left_stick_up_bias: StickBias::DISABLED,
    }
}

impl Record for keys::Settings {
    const ITEM: u8 = 0x10;
    const VERSION: u8 = 1;
}

impl Record for Keymaps {
    const ITEM: u8 = 0x11;
    const VERSION: u8 = 1;
}

impl Record for Overrides {
    const ITEM: u8 = 0x12;
    const VERSION: u8 = 1;
}

impl Record for advanced::Keys {
    const ITEM: u8 = 0x13;
    const VERSION: u8 = 1;
}

impl Record for gamepad::Mapping {
    const ITEM: u8 = 0x14;
    const VERSION: u8 = 1;
}

impl Record for gamepad::Settings {
    const ITEM: u8 = 0x15;
    const VERSION: u8 = 2;
}

impl Record for ActiveProfile {
    const ITEM: u8 = 0x10;
    const VERSION: u8 = 1;
}
