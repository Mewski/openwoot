use super::{Curve, Points, SwitchModel};

const TRAVEL_MAX: u16 = 4095;
const MIN_BOTTOM_DISTANCE: u32 = 50;
const TRAVEL_PER_TENTH_MILLIMETRE_UNITS: u32 = 0x1FFE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    kind: SwitchModel,
    travel_offset: Option<u8>,
    curve: Curve,
    curve_end: u32,
    learn_strength: u8,
    nominal_bottom: u16,
}

impl Switch {
    pub(crate) const REST_MARGIN: u16 = 5;

    #[must_use]
    pub fn new(kind: SwitchModel, travel_offset: Option<u8>) -> Self {
        let Some(offset) = travel_offset else {
            return Self::untuned(kind);
        };
        let strength = u32::from(kind.learn_strength());
        let travel = u32::from(kind.travel());
        let remaining = u32::from(kind.travel().saturating_sub(offset));
        let target = (Curve::VALUE_MAX * remaining / travel) & 0xFFFF;
        Self {
            travel_offset,
            curve_end: kind.curve().compact().end_of_travel(target),
            learn_strength: (strength * remaining / travel) as u8,
            ..Self::untuned(kind)
        }
    }

    #[must_use]
    pub(crate) const fn untuned(kind: SwitchModel) -> Self {
        Self {
            kind,
            travel_offset: None,
            curve: kind.curve(),
            curve_end: Curve::END_MAX,
            learn_strength: kind.learn_strength(),
            nominal_bottom: kind.nominal_bottom(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> SwitchModel {
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
    pub(crate) const fn lowest_bottom_level(rest_level: u16) -> u16 {
        rest_level.saturating_add(MIN_BOTTOM_DISTANCE as u16)
    }

    #[must_use]
    pub fn travel(&self, value: u16, rest_level: u16, bottom_level: u16) -> u16 {
        self.travel_over(self.curve.compact(), value, rest_level, bottom_level)
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

    #[must_use]
    pub(super) const fn curve(&self) -> Curve {
        self.curve
    }

    #[must_use]
    pub(super) fn travel_over(
        &self,
        points: &impl Points,
        value: u16,
        rest_level: u16,
        bottom_level: u16,
    ) -> u16 {
        if value <= rest_level.wrapping_add(Self::REST_MARGIN) {
            return 0;
        }
        let rest = u32::from(rest_level);
        let range = u32::from(bottom_level.saturating_sub(rest_level));
        let index_q8 = ((range >> 1).wrapping_add(
            self.curve_end
                .wrapping_mul(u32::from(value).wrapping_sub(rest)),
        ))
        .checked_div(range)
        .unwrap_or(0)
        .min(self.curve_end);
        let mut travel = u32::from(points.interpolate(index_q8));
        if self.travel_offset.is_some() {
            let end_value = u32::from(points.interpolate(self.curve_end));
            travel = (Curve::VALUE_MAX * travel)
                .checked_div(end_value)
                .unwrap_or(0)
                .min(Curve::VALUE_MAX);
        }
        (u32::from(TRAVEL_MAX) * travel / Curve::VALUE_MAX) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untuned_switch_is_the_switch_without_a_travel_offset() {
        for id in 0..SwitchModel::COUNT {
            let kind = SwitchModel::new(id).unwrap();
            assert_eq!(Switch::untuned(kind), Switch::new(kind, None), "{id}");
        }
    }
}
