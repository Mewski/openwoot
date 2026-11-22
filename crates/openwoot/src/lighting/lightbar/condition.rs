use openwoot_protocol::lightbar::{Comparator, Comparison, Condition, Group, Logic, Operand, Term};

use super::state::{ConditionState, TermState};
use crate::lighting::truncate::low_16;

pub(super) struct Outcome {
    pub(super) shown: bool,
    pub(super) changed: bool,
}

pub(super) fn evaluate(
    condition: &Condition,
    state: &mut ConditionState,
    elapsed_ms: u32,
) -> Outcome {
    let groups = condition.groups.iter().zip(state.terms.iter_mut());
    let result = match condition.logic {
        Logic::All => groups.fold(true, |result, (group, terms)| {
            evaluate_group(group, terms, elapsed_ms) & result
        }),
        Logic::Any if !condition.groups.is_empty() => groups
            .fold(false, |result, (group, terms)| {
                evaluate_group(group, terms, elapsed_ms) | result
            }),
        Logic::Single if condition.groups.len() == 1 => groups
            .map(|(group, terms)| evaluate_group(group, terms, elapsed_ms))
            .next()
            .unwrap_or(false),
        Logic::Any | Logic::Single => {
            state.result = false;
            state.dirty = false;
            return Outcome {
                shown: false,
                changed: false,
            };
        }
    };
    state.dirty = false;
    let previous = state.result;
    state.result = result;
    Outcome {
        shown: result,
        changed: result != previous,
    }
}

fn evaluate_group(group: &Group, states: &mut [TermState], elapsed_ms: u32) -> bool {
    let mut terms = group.terms.iter().zip(states.iter_mut());
    match group.logic {
        Logic::All => terms.fold(true, |result, (term, state)| {
            evaluate_term(term, state, elapsed_ms) & result
        }),
        Logic::Any => terms.fold(false, |result, (term, state)| {
            evaluate_term(term, state, elapsed_ms) | result
        }),
        Logic::Single if group.terms.len() == 1 => terms
            .next()
            .is_some_and(|(term, state)| evaluate_term(term, state, elapsed_ms)),
        Logic::Single => false,
    }
}

fn evaluate_term(term: &Term, state: &mut TermState, elapsed_ms: u32) -> bool {
    if state.hold_ms != 0 {
        state.hold_ms = low_16(u32::from(state.hold_ms).saturating_sub(elapsed_ms));
    }
    if !state.changed {
        return apply_hold(term, state, state.active);
    }
    match term {
        Term::Trigger { duration_ms, .. } => {
            state.hold_ms = *duration_ms;
            state.active = true;
            state.changed = false;
            apply_hold(term, state, true)
        }
        Term::Compare(comparison) => compare(term, comparison, state),
    }
}

#[expect(
    clippy::float_cmp,
    reason = "the condition terms compare their values exactly"
)]
fn compare(term: &Term, comparison: &Comparison, state: &mut TermState) -> bool {
    let left = state.left;
    let right = match comparison.operand {
        Operand::Signal(_) => state.right,
        Operand::Constant(constant) => constant,
    };
    let result = match comparison.comparator {
        Comparator::Equal => left == right,
        Comparator::NotEqual => left != right,
        Comparator::Greater => right < left,
        Comparator::GreaterOrEqual => right <= left,
        Comparator::Less => right > left,
        Comparator::LessOrEqual => right >= left,
    };
    if state.active == result {
        if result && state.hold_ms == 0 {
            return comparison.hold_ms.is_none();
        }
        return result;
    }
    if !result {
        state.active = false;
        state.changed = false;
        return false;
    }
    if let Some(hold_ms) = comparison.hold_ms {
        state.hold_ms = low_16(hold_ms);
    }
    state.active = true;
    state.changed = false;
    apply_hold(term, state, true)
}

fn apply_hold(term: &Term, state: &TermState, result: bool) -> bool {
    let timed = match term {
        Term::Trigger { .. } => true,
        Term::Compare(comparison) => comparison.hold_ms.is_some(),
    };
    if timed && result {
        state.hold_ms != 0
    } else {
        result
    }
}

pub(super) fn is_timed(condition: &Condition) -> bool {
    condition
        .groups
        .iter()
        .flat_map(|group| group.terms.iter())
        .any(|term| match term {
            Term::Trigger { .. } => true,
            Term::Compare(comparison) => comparison.hold_ms.is_some(),
        })
}

pub(super) fn needs_render(condition: &Condition, state: &ConditionState, elapsed_ms: u16) -> bool {
    if !state.dirty && !state.timed {
        return false;
    }
    condition
        .groups
        .iter()
        .zip(&state.terms)
        .flat_map(|(group, terms)| terms.iter().take(group.terms.len()))
        .any(|term| (term.hold_ms != 0 && elapsed_ms > term.hold_ms) || term.changed)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        heapless::Vec,
        lightbar::{Builtin, Signal},
    };

    use super::*;

    const SIGNAL: Signal = Signal::Builtin(Builtin::KeyRate);

    fn single(term: Term) -> Condition {
        Condition {
            logic: Logic::Single,
            groups: Vec::from_iter([Group {
                logic: Logic::Single,
                terms: Vec::from_iter([term]),
            }]),
        }
    }

    fn greater(constant: f32, hold_ms: Option<u32>) -> Term {
        Term::Compare(Comparison {
            signal: SIGNAL,
            comparator: Comparator::Greater,
            operand: Operand::Constant(constant),
            hold_ms,
        })
    }

    fn input(state: &mut ConditionState, value: f32) {
        state.terms[0][0].left = value;
        state.terms[0][0].changed = true;
        state.dirty = true;
    }

    #[test]
    fn a_trigger_holds_for_its_duration_after_each_change() {
        let condition = single(Term::Trigger {
            signal: SIGNAL,
            duration_ms: 100,
        });
        let mut state = ConditionState::default();
        assert!(!evaluate(&condition, &mut state, 16).shown);
        input(&mut state, 1.0);
        let first = evaluate(&condition, &mut state, 16);
        assert!(first.shown && first.changed);
        assert!(evaluate(&condition, &mut state, 60).shown);
        assert!(!evaluate(&condition, &mut state, 60).shown);
    }

    #[test]
    fn a_comparison_with_hold_shows_once_per_rising_edge() {
        let condition = single(greater(5.0, Some(50)));
        let mut state = ConditionState::default();
        input(&mut state, 6.0);
        assert!(evaluate(&condition, &mut state, 16).shown);
        input(&mut state, 7.0);
        assert!(evaluate(&condition, &mut state, 16).shown);
        assert!(!evaluate(&condition, &mut state, 40).shown);
        input(&mut state, 8.0);
        assert!(!evaluate(&condition, &mut state, 16).shown);
        input(&mut state, 1.0);
        assert!(!evaluate(&condition, &mut state, 16).shown);
        input(&mut state, 9.0);
        assert!(evaluate(&condition, &mut state, 16).shown);
    }

    #[test]
    fn a_comparison_without_hold_follows_its_input() {
        let condition = single(greater(5.0, None));
        let mut state = ConditionState::default();
        input(&mut state, 6.0);
        assert!(evaluate(&condition, &mut state, 16).shown);
        assert!(evaluate(&condition, &mut state, 16).shown);
        input(&mut state, 5.0);
        let falling = evaluate(&condition, &mut state, 16);
        assert!(!falling.shown && falling.changed);
    }

    #[test]
    fn an_any_condition_without_groups_hides_the_entry() {
        let condition = Condition {
            logic: Logic::Any,
            groups: Vec::new(),
        };
        let mut state = ConditionState::default();
        assert!(!evaluate(&condition, &mut state, 16).shown);
        let all = Condition {
            logic: Logic::All,
            groups: Vec::new(),
        };
        assert!(evaluate(&all, &mut state, 16).shown);
    }
}
