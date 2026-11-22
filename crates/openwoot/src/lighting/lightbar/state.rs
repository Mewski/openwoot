use openwoot_protocol::{
    lightbar::{MAX_GROUPS, MAX_TERMS},
    lighting::Rgb,
};

use crate::lighting::color::BLACK;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct TermState {
    pub(super) active: bool,
    pub(super) changed: bool,
    pub(super) left: f32,
    pub(super) right: f32,
    pub(super) hold_ms: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ConditionState {
    pub(super) result: bool,
    pub(super) dirty: bool,
    pub(super) timed: bool,
    pub(super) terms: [[TermState; MAX_TERMS]; MAX_GROUPS],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct IndicatorState {
    pub(super) profile: f32,
    pub(super) count: f32,
    pub(super) dirty: bool,
    pub(super) color: Rgb,
    pub(super) background: Option<Rgb>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum EffectState {
    #[default]
    Stateless,
    Meter {
        value: f32,
        dirty: bool,
    },
    Indicator(IndicatorState),
    Blink {
        phase_ms: u16,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct EntryState {
    pub(super) bound: bool,
    pub(super) condition: ConditionState,
    pub(super) effect: EffectState,
}

impl TermState {
    pub(super) const fn new() -> Self {
        Self {
            active: false,
            changed: false,
            left: 0.0,
            right: 0.0,
            hold_ms: 0,
        }
    }
}

impl ConditionState {
    pub(super) const fn new() -> Self {
        Self {
            result: false,
            dirty: false,
            timed: false,
            terms: [[TermState::new(); MAX_TERMS]; MAX_GROUPS],
        }
    }

    pub(super) fn term_mut(&mut self, group: u8, term: u8) -> Option<&mut TermState> {
        self.terms
            .get_mut(usize::from(group))
            .and_then(|terms| terms.get_mut(usize::from(term)))
    }
}

impl EntryState {
    pub(super) const fn new() -> Self {
        Self {
            bound: false,
            condition: ConditionState::new(),
            effect: EffectState::Stateless,
        }
    }
}

impl IndicatorState {
    pub(super) const fn new() -> Self {
        Self {
            profile: 0.0,
            count: 0.0,
            dirty: true,
            color: BLACK,
            background: None,
        }
    }
}
