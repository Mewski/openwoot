use super::curve::{CURVE_END_MAX, CURVE_VALUE_MAX, Curve};

pub(crate) const REST_MARGIN: u16 = 5;
pub(crate) const TRAVEL_MAX: u16 = 4095;

const MIN_BOTTOM_DISTANCE: u32 = 50;
const TRAVEL_PER_TENTH_MILLIMETRE_UNITS: u32 = 0x1FFE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwitchType(u8);

impl SwitchType {
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
    pub fn travel(self) -> u8 {
        self.specification().travel
    }

    #[must_use]
    pub fn group(self) -> u8 {
        self.specification().group
    }

    #[must_use]
    pub fn learn_strength(self) -> u8 {
        self.specification().learn_strength
    }

    #[must_use]
    pub fn nominal_bottom(self) -> u16 {
        self.specification().nominal_bottom
    }

    #[must_use]
    pub fn curve(self) -> Curve {
        Curve::number(self.specification().curve)
    }

    fn specification(self) -> &'static Specification {
        match SPECIFICATIONS.get(usize::from(self.0)) {
            Some(specification) => specification,
            None => &SPECIFICATIONS[0],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    kind: SwitchType,
    travel_offset: Option<u8>,
    curve: Curve,
    curve_end: u32,
    learn_strength: u8,
    nominal_bottom: u16,
}

impl Switch {
    #[must_use]
    pub fn new(kind: SwitchType, travel_offset: Option<u8>) -> Self {
        let curve = kind.curve();
        let strength = u32::from(kind.learn_strength());
        let (curve_end, learn_strength) = match travel_offset {
            None => (CURVE_END_MAX, strength),
            Some(offset) => {
                let travel = u32::from(kind.travel());
                let remaining = u32::from(kind.travel().wrapping_sub(offset));
                let target = (CURVE_VALUE_MAX * remaining / travel) & 0xFFFF;
                (curve.end_of_travel(target), strength * remaining / travel)
            }
        };
        Self {
            kind,
            travel_offset,
            curve,
            curve_end,
            learn_strength: learn_strength as u8,
            nominal_bottom: kind.nominal_bottom(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> SwitchType {
        self.kind
    }

    #[must_use]
    pub const fn travel_offset(&self) -> Option<u8> {
        self.travel_offset
    }

    #[must_use]
    pub const fn curve_end(&self) -> u32 {
        self.curve_end
    }

    #[must_use]
    pub const fn learn_strength(&self) -> u8 {
        self.learn_strength
    }

    #[must_use]
    pub const fn nominal_bottom(&self) -> u16 {
        self.nominal_bottom
    }

    #[must_use]
    pub fn bottom_level(&self, rest_level: u16) -> u16 {
        let span = u32::from(self.nominal_bottom.saturating_sub(rest_level));
        let scaled = self
            .curve_end
            .wrapping_mul(span)
            .wrapping_add(CURVE_END_MAX / 2);
        let distance = (scaled / CURVE_END_MAX).max(MIN_BOTTOM_DISTANCE);
        (u32::from(rest_level) + distance) as u16
    }

    #[must_use]
    pub fn travel(&self, value: u16, rest_level: u16, bottom_level: u16) -> u16 {
        if value <= rest_level.wrapping_add(REST_MARGIN) {
            return 0;
        }
        let rest = u32::from(rest_level);
        let range = u32::from(bottom_level.wrapping_sub(rest_level));
        let index_q8 = ((range >> 1).wrapping_add(
            self.curve_end
                .wrapping_mul(u32::from(value).wrapping_sub(rest)),
        ))
        .checked_div(range)
        .unwrap_or(0)
        .min(self.curve_end);
        let mut travel = u32::from(self.curve.interpolate(index_q8));
        if self.travel_offset.is_some() {
            let end_value = u32::from(self.curve.interpolate(self.curve_end));
            travel = (CURVE_VALUE_MAX * travel)
                .checked_div(end_value)
                .unwrap_or(0)
                .min(CURVE_VALUE_MAX);
        }
        (u32::from(TRAVEL_MAX) * travel / CURVE_VALUE_MAX) as u16
    }

    #[must_use]
    pub fn travel_per_tenth_millimetre(&self) -> u32 {
        let travel = self.kind.travel();
        let offset = self.travel_offset.unwrap_or(0);
        let remaining = travel.saturating_sub(offset);
        TRAVEL_PER_TENTH_MILLIMETRE_UNITS
            .checked_div(u32::from(remaining))
            .unwrap_or(0)
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

const SPECIFICATIONS: [Specification; SwitchType::COUNT as usize] = [
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
