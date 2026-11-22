use openwoot_protocol::lightbar::{
    Builtin, Comparison, Condition, Effect, Entry, Group, Logic, Operand, Signal, Term,
};

use super::{
    background::background_of,
    condition::is_timed,
    state::{ConditionState, EffectState, EntryState, IndicatorState, TermState},
};
use crate::lighting::signals::{Input, Subscriber, Table, allowed_in_fast_scan, color_of};

#[derive(Clone, Copy)]
pub(super) struct Origin {
    pub(super) global: bool,
    pub(super) entry: u8,
}

pub(super) fn bind(
    signals: &mut Table,
    origin: Origin,
    entry: &Entry,
    state: &mut EntryState,
    fast_scan: bool,
) -> bool {
    if fast_scan && !allowed_in_fast_scan_mode(entry) {
        state.bound = false;
        return false;
    }
    state.bound = true;
    let effect = origin.subscriber(Input::Effect);
    state.effect = match &entry.effect {
        Effect::Meter(meter) => EffectState::Meter {
            value: signals.subscribe(meter.signal, effect),
            dirty: false,
        },
        Effect::ProfileIndicator(indicator) => {
            let mut view = IndicatorState::new();
            view.profile = signals.subscribe(Signal::Builtin(Builtin::ActiveProfile), effect);
            view.count = signals.subscribe(Signal::Builtin(Builtin::ProfileCount), effect);
            view.color =
                color_of(signals.subscribe(Signal::Builtin(Builtin::ProfileColor), effect));
            view.background = background_of(indicator.background, view.color);
            EffectState::Indicator(view)
        }
        Effect::Blink(_) => EffectState::Blink { phase_ms: 0 },
        Effect::Mirror(_) | Effect::Solid(_) | Effect::Custom(_) => EffectState::Stateless,
    };
    if let Some(condition) = &entry.condition {
        bind_condition(signals, origin, condition, &mut state.condition);
    }
    true
}

fn bind_condition(
    signals: &mut Table,
    origin: Origin,
    condition: &Condition,
    state: &mut ConditionState,
) {
    *state = ConditionState::default();
    let bound = match condition.logic {
        Logic::Single if condition.groups.len() == 1 => {
            if let (Some(group), Some(terms)) = (condition.groups.first(), state.terms.first_mut())
            {
                bind_group(signals, origin, 0, group, terms);
            }
            true
        }
        Logic::Single => false,
        Logic::All | Logic::Any => true,
    };
    if bound {
        if condition.groups.is_empty() {
            state.dirty = true;
            return;
        }
        let mut all_builtin = true;
        for ((group, terms), number) in condition.groups.iter().zip(&mut state.terms).zip(0..) {
            all_builtin &= bind_group(signals, origin, number, group, terms);
        }
        if all_builtin {
            state.dirty = true;
        }
    }
    state.timed = is_timed(condition);
}

fn bind_group(
    signals: &mut Table,
    origin: Origin,
    group_number: u8,
    group: &Group,
    states: &mut [TermState],
) -> bool {
    let subscriber = |term| {
        origin.subscriber(Input::Condition {
            group: group_number,
            term,
            right: false,
        })
    };
    match group.logic {
        Logic::Single => match (group.terms.as_slice(), states.first_mut()) {
            ([Term::Trigger { signal, .. }], _) => {
                signals.subscribe(*signal, subscriber(0));
                true
            }
            ([Term::Compare(comparison)], Some(state)) => {
                bind_comparison(signals, subscriber(0), comparison, state)
            }
            _ => false,
        },
        Logic::All | Logic::Any => {
            let mut all_builtin = true;
            for ((term, state), number) in group.terms.iter().zip(states.iter_mut()).zip(0..) {
                match term {
                    Term::Trigger { signal, .. } => {
                        signals.subscribe(*signal, subscriber(number));
                    }
                    Term::Compare(comparison) => {
                        all_builtin &=
                            bind_comparison(signals, subscriber(number), comparison, state);
                    }
                }
            }
            all_builtin
        }
    }
}

fn bind_comparison(
    signals: &mut Table,
    subscriber: Subscriber,
    comparison: &Comparison,
    state: &mut TermState,
) -> bool {
    let value = signals.subscribe(comparison.signal, subscriber);
    let left_builtin = matches!(comparison.signal, Signal::Builtin(_));
    if left_builtin {
        state.left = value;
    }
    match comparison.operand {
        Operand::Signal(signal) => {
            let right = Subscriber {
                input: match subscriber.input {
                    Input::Condition { group, term, .. } => Input::Condition {
                        group,
                        term,
                        right: true,
                    },
                    Input::Effect => Input::Effect,
                },
                ..subscriber
            };
            let value = signals.subscribe(signal, right);
            if !matches!(signal, Signal::Builtin(_)) {
                return false;
            }
            state.right = value;
            if !left_builtin {
                return false;
            }
        }
        Operand::Constant(_) => {
            if !left_builtin {
                return false;
            }
        }
    }
    state.changed = true;
    true
}

fn allowed_in_fast_scan_mode(entry: &Entry) -> bool {
    if let Some(condition) = &entry.condition {
        let groups_allowed = match condition.logic {
            Logic::Single => condition.groups.len() == 1,
            Logic::All | Logic::Any => true,
        };
        if !groups_allowed || !condition.groups.iter().all(group_allowed_in_fast_scan) {
            return false;
        }
    }
    match &entry.effect {
        Effect::ProfileIndicator(_) | Effect::Mirror(_) | Effect::Solid(_) | Effect::Custom(_) => {
            true
        }
        Effect::Meter(meter) => allowed_in_fast_scan(meter.signal),
        Effect::Blink(_) => false,
    }
}

fn group_allowed_in_fast_scan(group: &Group) -> bool {
    let terms_allowed = match group.logic {
        Logic::Single => group.terms.len() == 1,
        Logic::All | Logic::Any => true,
    };
    terms_allowed
        && group.terms.iter().all(|term| match term {
            Term::Trigger { signal, .. } => allowed_in_fast_scan(*signal),
            Term::Compare(comparison) => allowed_in_fast_scan(comparison.signal),
        })
}

impl Origin {
    const fn subscriber(self, input: Input) -> Subscriber {
        Subscriber {
            global: self.global,
            entry: self.entry,
            input,
        }
    }
}
