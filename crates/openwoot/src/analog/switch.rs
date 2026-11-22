use super::{Curve, SwitchType};

const TRAVEL_MAX: u16 = 4095;
const MIN_BOTTOM_DISTANCE: u32 = 50;
const TRAVEL_PER_TENTH_MILLIMETRE_UNITS: u32 = 0x1FFE;

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
    pub(crate) const REST_MARGIN: u16 = 5;

    #[must_use]
    pub fn new(kind: SwitchType, travel_offset: Option<u8>) -> Self {
        let curve = kind.curve();
        let strength = u32::from(kind.learn_strength());
        let (curve_end, learn_strength) = match travel_offset {
            None => (Curve::END_MAX, strength),
            Some(offset) => {
                let travel = u32::from(kind.travel());
                let remaining = u32::from(kind.travel().wrapping_sub(offset));
                let target = (Curve::VALUE_MAX * remaining / travel) & 0xFFFF;
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
            .wrapping_add(Curve::END_MAX / 2);
        let distance = (scaled / Curve::END_MAX).max(MIN_BOTTOM_DISTANCE);
        (u32::from(rest_level) + distance) as u16
    }

    #[must_use]
    pub fn travel(&self, value: u16, rest_level: u16, bottom_level: u16) -> u16 {
        if value <= rest_level.wrapping_add(Self::REST_MARGIN) {
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
            travel = (Curve::VALUE_MAX * travel)
                .checked_div(end_value)
                .unwrap_or(0)
                .min(Curve::VALUE_MAX);
        }
        (u32::from(TRAVEL_MAX) * travel / Curve::VALUE_MAX) as u16
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
