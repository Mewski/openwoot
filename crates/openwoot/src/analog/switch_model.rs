use super::Curve;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwitchModel(u8);

impl SwitchModel {
    pub const COUNT: u8 = 49;
    pub const DEFAULT: Self = Self(0);

    #[must_use]
    pub const fn new(id: u8) -> Option<Self> {
        if id < Self::COUNT {
            Some(Self(id))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn travel(self) -> u8 {
        self.specification().travel
    }

    #[must_use]
    pub const fn group(self) -> u8 {
        self.specification().group
    }

    #[must_use]
    pub const fn learn_strength(self) -> u8 {
        self.specification().learn_strength
    }

    #[must_use]
    pub const fn nominal_bottom(self) -> u16 {
        self.specification().nominal_bottom
    }

    #[must_use]
    pub(super) const fn curve(self) -> Curve {
        Curve::number(self.specification().curve)
    }

    const fn specification(self) -> &'static Specification {
        match SPECIFICATIONS.split_at_checked(self.0 as usize) {
            Some((_, [specification, ..])) => specification,
            _ => &SPECIFICATIONS[0],
        }
    }
}

#[derive(Debug)]
struct Specification {
    travel: u8,
    group: u8,
    learn_strength: u8,
    curve: u8,
    nominal_bottom: u16,
}

const SPECIFICATIONS: [Specification; SwitchModel::COUNT as usize] = [
    Specification {
        travel: 80,
        group: 0,
        learn_strength: 255,
        curve: 0,
        nominal_bottom: 1200,
    },
    Specification {
        travel: 80,
        group: 0,
        learn_strength: 70,
        curve: 1,
        nominal_bottom: 1250,
    },
    Specification {
        travel: 66,
        group: 0,
        learn_strength: 44,
        curve: 2,
        nominal_bottom: 900,
    },
    Specification {
        travel: 68,
        group: 7,
        learn_strength: 57,
        curve: 3,
        nominal_bottom: 1000,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 59,
        curve: 4,
        nominal_bottom: 950,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 59,
        curve: 5,
        nominal_bottom: 950,
    },
    Specification {
        travel: 70,
        group: 6,
        learn_strength: 59,
        curve: 5,
        nominal_bottom: 950,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 59,
        curve: 4,
        nominal_bottom: 950,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 59,
        curve: 5,
        nominal_bottom: 950,
    },
    Specification {
        travel: 66,
        group: 2,
        learn_strength: 38,
        curve: 6,
        nominal_bottom: 930,
    },
    Specification {
        travel: 68,
        group: 3,
        learn_strength: 63,
        curve: 7,
        nominal_bottom: 900,
    },
    Specification {
        travel: 64,
        group: 5,
        learn_strength: 51,
        curve: 8,
        nominal_bottom: 700,
    },
    Specification {
        travel: 68,
        group: 11,
        learn_strength: 45,
        curve: 9,
        nominal_bottom: 1000,
    },
    Specification {
        travel: 66,
        group: 11,
        learn_strength: 63,
        curve: 10,
        nominal_bottom: 1000,
    },
    Specification {
        travel: 70,
        group: 14,
        learn_strength: 114,
        curve: 11,
        nominal_bottom: 1000,
    },
    Specification {
        travel: 70,
        group: 16,
        learn_strength: 79,
        curve: 12,
        nominal_bottom: 1250,
    },
    Specification {
        travel: 64,
        group: 4,
        learn_strength: 63,
        curve: 13,
        nominal_bottom: 720,
    },
    Specification {
        travel: 68,
        group: 3,
        learn_strength: 63,
        curve: 14,
        nominal_bottom: 930,
    },
    Specification {
        travel: 66,
        group: 10,
        learn_strength: 42,
        curve: 15,
        nominal_bottom: 1100,
    },
    Specification {
        travel: 68,
        group: 7,
        learn_strength: 96,
        curve: 16,
        nominal_bottom: 930,
    },
    Specification {
        travel: 68,
        group: 1,
        learn_strength: 44,
        curve: 17,
        nominal_bottom: 950,
    },
    Specification {
        travel: 70,
        group: 6,
        learn_strength: 79,
        curve: 18,
        nominal_bottom: 850,
    },
    Specification {
        travel: 69,
        group: 6,
        learn_strength: 49,
        curve: 19,
        nominal_bottom: 950,
    },
    Specification {
        travel: 64,
        group: 13,
        learn_strength: 51,
        curve: 20,
        nominal_bottom: 950,
    },
    Specification {
        travel: 70,
        group: 8,
        learn_strength: 20,
        curve: 21,
        nominal_bottom: 1450,
    },
    Specification {
        travel: 68,
        group: 9,
        learn_strength: 33,
        curve: 22,
        nominal_bottom: 1150,
    },
    Specification {
        travel: 68,
        group: 11,
        learn_strength: 96,
        curve: 23,
        nominal_bottom: 970,
    },
    Specification {
        travel: 70,
        group: 15,
        learn_strength: 57,
        curve: 24,
        nominal_bottom: 1250,
    },
    Specification {
        travel: 68,
        group: 7,
        learn_strength: 96,
        curve: 16,
        nominal_bottom: 930,
    },
    Specification {
        travel: 68,
        group: 8,
        learn_strength: 96,
        curve: 16,
        nominal_bottom: 930,
    },
    Specification {
        travel: 66,
        group: 4,
        learn_strength: 38,
        curve: 6,
        nominal_bottom: 930,
    },
    Specification {
        travel: 64,
        group: 12,
        learn_strength: 51,
        curve: 20,
        nominal_bottom: 950,
    },
    Specification {
        travel: 66,
        group: 14,
        learn_strength: 44,
        curve: 2,
        nominal_bottom: 900,
    },
    Specification {
        travel: 70,
        group: 2,
        learn_strength: 44,
        curve: 25,
        nominal_bottom: 1200,
    },
    Specification {
        travel: 80,
        group: 4,
        learn_strength: 255,
        curve: 26,
        nominal_bottom: 1000,
    },
    Specification {
        travel: 76,
        group: 10,
        learn_strength: 95,
        curve: 27,
        nominal_bottom: 1050,
    },
    Specification {
        travel: 68,
        group: 7,
        learn_strength: 42,
        curve: 28,
        nominal_bottom: 1200,
    },
    Specification {
        travel: 70,
        group: 0,
        learn_strength: 45,
        curve: 29,
        nominal_bottom: 1050,
    },
    Specification {
        travel: 64,
        group: 0,
        learn_strength: 51,
        curve: 30,
        nominal_bottom: 1070,
    },
    Specification {
        travel: 70,
        group: 0,
        learn_strength: 22,
        curve: 31,
        nominal_bottom: 1300,
    },
    Specification {
        travel: 64,
        group: 0,
        learn_strength: 45,
        curve: 32,
        nominal_bottom: 800,
    },
    Specification {
        travel: 80,
        group: 7,
        learn_strength: 108,
        curve: 33,
        nominal_bottom: 1120,
    },
    Specification {
        travel: 68,
        group: 9,
        learn_strength: 20,
        curve: 34,
        nominal_bottom: 1300,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 31,
        curve: 35,
        nominal_bottom: 1170,
    },
    Specification {
        travel: 68,
        group: 6,
        learn_strength: 43,
        curve: 35,
        nominal_bottom: 1170,
    },
    Specification {
        travel: 70,
        group: 10,
        learn_strength: 39,
        curve: 36,
        nominal_bottom: 1230,
    },
    Specification {
        travel: 70,
        group: 11,
        learn_strength: 22,
        curve: 37,
        nominal_bottom: 1200,
    },
    Specification {
        travel: 68,
        group: 11,
        learn_strength: 22,
        curve: 38,
        nominal_bottom: 1200,
    },
    Specification {
        travel: 68,
        group: 11,
        learn_strength: 30,
        curve: 39,
        nominal_bottom: 1150,
    },
];
