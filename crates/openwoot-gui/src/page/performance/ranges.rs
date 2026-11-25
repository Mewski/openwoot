pub(super) const STEP_MM: f32 = 0.05;
pub(super) const RELEASE_DEFAULT_MM: f32 = 0.55;
const MIN_MM: f32 = 0.1;
const PRESS_DEFAULT_MM: f32 = 0.4;
const SPLIT_PRESS_DEFAULT_MM: f32 = 0.2;
const SENSITIVITY_SHARE: f32 = 0.6;
const SENSITIVITY_HEADROOM_MM: f32 = 0.5;
const SPACER_HEADROOM_MM: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Span {
    pub(crate) min: f32,
    pub(crate) max: f32,
}

impl Span {
    pub(crate) const fn actuation(travel: f32) -> Self {
        Self {
            min: MIN_MM,
            max: travel.max(MIN_MM),
        }
    }

    pub(crate) fn sensitivity(travel: f32) -> Self {
        let limit = ((travel - STEP_MM) * SENSITIVITY_SHARE).min(travel - SENSITIVITY_HEADROOM_MM);
        Self {
            min: MIN_MM,
            max: Self::actuation(travel).snap(limit),
        }
    }

    pub(crate) const fn spacer(travel: f32) -> Self {
        Self {
            min: STEP_MM,
            max: (travel - SPACER_HEADROOM_MM).max(STEP_MM),
        }
    }

    pub(crate) fn snap(self, millimetres: f32) -> f32 {
        let steps = ((millimetres.clamp(self.min, self.max) - self.min) / STEP_MM).round();
        (self.min + steps * STEP_MM).min(self.max)
    }

    pub(crate) fn fraction(self, millimetres: f32) -> f32 {
        let length = self.max - self.min;
        if length > 0.0 {
            ((millimetres - self.min) / length).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

pub(super) const fn press_default(split: bool) -> f32 {
    if split {
        SPLIT_PRESS_DEFAULT_MM
    } else {
        PRESS_DEFAULT_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(first: f32, second: f32) -> bool {
        (first - second).abs() < 1e-4
    }

    #[test]
    fn the_actuation_point_spans_the_whole_travel() {
        let span = Span::actuation(4.0);
        assert!(close(span.min, 0.1));
        assert!(close(span.max, 4.0));
    }

    #[test]
    fn rapid_trigger_sensitivity_stops_where_wootility_stops() {
        assert!(close(Span::sensitivity(4.0).max, 2.35));
        assert!(close(Span::sensitivity(3.3).max, 1.95));
        assert!(close(Span::sensitivity(3.5).max, 2.05));
    }

    #[test]
    fn values_snap_to_steps_of_five_hundredths_inside_the_span() {
        let span = Span::actuation(4.0);
        assert!(close(span.snap(0.524), 0.5));
        assert!(close(span.snap(0.53), 0.55));
        assert!(close(span.snap(-1.0), 0.1));
        assert!(close(span.snap(9.0), 4.0));
        assert!(close(span.fraction(2.05), 0.5));
        assert!(close(span.fraction(9.0), 1.0));
    }

    #[test]
    fn split_sensitivity_starts_lower_than_a_single_one() {
        assert!(press_default(true) < press_default(false));
        assert!(press_default(false) < RELEASE_DEFAULT_MM);
    }
}
