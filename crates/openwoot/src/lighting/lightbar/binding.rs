use openwoot_protocol::lightbar::{
    Builtin, Comparison, Condition, Effect, Entry, Group, Logic, Operand, Signal, Term,
};

use super::{
    background::background_of,
    condition::is_timed,
    state::{ConditionState, EffectState, EntryState, IndicatorState, TermState},
};
use crate::lighting::signals::{Subscriber, Table, allowed_in_fast_scan, color_of};

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
    let subscriber = origin.subscriber();
    state.effect = match &entry.effect {
        Effect::Meter(meter) => EffectState::Meter {
            value: signals.subscribe(meter.signal, subscriber).unwrap_or(0.0),
            dirty: false,
        },
        Effect::ProfileIndicator(indicator) => {
            let mut current = |builtin: Builtin| {
                signals
                    .subscribe(Signal::Builtin(builtin), subscriber)
                    .unwrap_or(0.0)
            };
            let mut view = IndicatorState::new();
            view.profile = current(Builtin::ActiveProfile);
            view.count = current(Builtin::ProfileCount);
            view.color = color_of(current(Builtin::ProfileColor));
            view.background = background_of(indicator.background, view.color);
            EffectState::Indicator(view)
        }
        Effect::Blink(_) => EffectState::Blink { phase_ms: 0 },
        Effect::Mirror(_) | Effect::Solid(_) | Effect::Custom(_) => EffectState::Stateless,
    };
    if let Some(condition) = &entry.condition {
        bind_condition(signals, subscriber, condition, &mut state.condition);
    }
    true
}

pub(super) fn notify_condition(
    condition: &Condition,
    state: &mut ConditionState,
    signal: Signal,
    value: f32,
) {
    if !binds_groups(condition) {
        return;
    }
    let mut heard = false;
    for (group, terms) in condition.groups.iter().zip(&mut state.terms) {
        if binds_terms(group) {
            for (term, term_state) in group.terms.iter().zip(terms.iter_mut()) {
                heard |= notify_term(term, term_state, signal, value);
            }
        }
    }
    if heard {
        state.dirty = true;
    }
}

fn notify_term(term: &Term, state: &mut TermState, signal: Signal, value: f32) -> bool {
    let (left, right) = match term {
        Term::Trigger { signal, .. } => (*signal, None),
        Term::Compare(comparison) => (comparison.signal, operand_signal(comparison)),
    };
    let mut heard = false;
    if left == signal {
        state.left = value;
        heard = true;
    }
    if right == Some(signal) {
        state.right = value;
        heard = true;
    }
    if heard {
        state.changed = true;
    }
    heard
}

fn bind_condition(
    signals: &mut Table,
    subscriber: Subscriber,
    condition: &Condition,
    state: &mut ConditionState,
) {
    *state = ConditionState::default();
    if binds_groups(condition) {
        if condition.groups.is_empty() {
            state.dirty = true;
            return;
        }
        let mut known = true;
        for (group, terms) in condition.groups.iter().zip(&mut state.terms) {
            known &= bind_group(signals, subscriber, group, terms);
        }
        if known {
            state.dirty = true;
        }
    }
    state.timed = is_timed(condition);
}

fn bind_group(
    signals: &mut Table,
    subscriber: Subscriber,
    group: &Group,
    states: &mut [TermState],
) -> bool {
    if !binds_terms(group) {
        return false;
    }
    let mut known = true;
    for (term, state) in group.terms.iter().zip(states.iter_mut()) {
        match term {
            Term::Trigger { signal, .. } => {
                signals.subscribe(*signal, subscriber);
            }
            Term::Compare(comparison) => {
                known &= bind_comparison(signals, subscriber, comparison, state);
            }
        }
    }
    known
}

fn bind_comparison(
    signals: &mut Table,
    subscriber: Subscriber,
    comparison: &Comparison,
    state: &mut TermState,
) -> bool {
    let mut known = true;
    match signals.subscribe(comparison.signal, subscriber) {
        Some(value) => state.left = value,
        None => known = false,
    }
    if let Some(signal) = operand_signal(comparison) {
        match signals.subscribe(signal, subscriber) {
            Some(value) => state.right = value,
            None => known = false,
        }
    }
    if known {
        state.changed = true;
    }
    known
}

fn operand_signal(comparison: &Comparison) -> Option<Signal> {
    match comparison.operand {
        Operand::Signal(signal) => Some(signal),
        Operand::Constant(_) => None,
    }
}

fn binds_groups(condition: &Condition) -> bool {
    match condition.logic {
        Logic::Single => condition.groups.len() == 1,
        Logic::All | Logic::Any => true,
    }
}

fn binds_terms(group: &Group) -> bool {
    match group.logic {
        Logic::Single => group.terms.len() == 1,
        Logic::All | Logic::Any => true,
    }
}

fn allowed_in_fast_scan_mode(entry: &Entry) -> bool {
    if let Some(condition) = &entry.condition
        && (!binds_groups(condition) || !condition.groups.iter().all(group_allowed_in_fast_scan))
    {
        return false;
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
    binds_terms(group)
        && group.terms.iter().all(|term| match term {
            Term::Trigger { signal, .. } => allowed_in_fast_scan(*signal),
            Term::Compare(comparison) => {
                allowed_in_fast_scan(comparison.signal)
                    && operand_signal(comparison).is_none_or(allowed_in_fast_scan)
            }
        })
}

impl Origin {
    const fn subscriber(self) -> Subscriber {
        Subscriber {
            global: self.global,
            entry: self.entry,
        }
    }
}
