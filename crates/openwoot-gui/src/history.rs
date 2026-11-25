use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{Context, Task};
use openwoot_protocol::{Endpoint, configuration::Rejection};
use serde::Serialize;

use crate::{
    device::Failure,
    store::{Scope, Store},
};

const MERGE_WINDOW: Duration = Duration::from_millis(800);
const DEPTH: usize = 200;

pub(crate) type Replay = Rc<dyn Fn(&Store, &mut Context<Store>) -> Task<Result<(), Failure>>>;

#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
}

#[derive(Clone)]
struct Step {
    path: &'static str,
    scope: Scope,
    before: Vec<u8>,
    after: Vec<u8>,
    restore: Replay,
    reapply: Replay,
    at: Instant,
}

impl History {
    pub(crate) fn record<E>(&mut self, before: &E::Request, after: &E::Request, scope: Scope)
    where
        E: Endpoint<Response = Result<(), Rejection>>,
        E::Request: Clone + Serialize + Send + 'static,
    {
        let (Ok(before_bytes), Ok(after_bytes)) =
            (serde_json::to_vec(before), serde_json::to_vec(after))
        else {
            return;
        };
        if before_bytes == after_bytes {
            return;
        }
        self.push(Step {
            path: E::PATH,
            scope,
            before: before_bytes,
            after: after_bytes,
            restore: sender::<E>(before.clone(), scope),
            reapply: sender::<E>(after.clone(), scope),
            at: Instant::now(),
        });
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(crate) fn undo(&mut self) -> Option<Replay> {
        let step = self.undo.pop()?;
        let restore = step.restore.clone();
        self.redo.push(step);
        Some(restore)
    }

    pub(crate) fn redo(&mut self) -> Option<Replay> {
        let step = self.redo.pop()?;
        let reapply = step.reapply.clone();
        self.undo.push(step);
        Some(reapply)
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    fn push(&mut self, step: Step) {
        self.redo.clear();
        if let Some(last) = self.undo.last_mut()
            && last.path == step.path
            && last.scope == step.scope
            && last.after == step.before
            && step.at.saturating_duration_since(last.at) < MERGE_WINDOW
        {
            last.after = step.after;
            last.reapply = step.reapply;
            last.at = step.at;
            if last.before == last.after {
                self.undo.pop();
            }
            return;
        }
        if self.undo.len() == DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(step);
    }
}

fn sender<E>(request: E::Request, scope: Scope) -> Replay
where
    E: Endpoint<Response = Result<(), Rejection>>,
    E::Request: Clone + Serialize + Send + 'static,
{
    Rc::new(move |store, cx| store.apply::<E>(request.clone(), scope, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(path: &'static str, before: u8, after: u8, at: Instant) -> Step {
        let idle: Replay = Rc::new(|_, _| Task::ready(Ok(())));
        Step {
            path,
            scope: Scope::Keyboard,
            before: vec![before],
            after: vec![after],
            restore: idle.clone(),
            reapply: idle,
            at,
        }
    }

    fn spans(history: &History) -> Vec<(Vec<u8>, Vec<u8>)> {
        history
            .undo
            .iter()
            .map(|step| (step.before.clone(), step.after.clone()))
            .collect()
    }

    #[test]
    fn a_quick_chain_of_changes_to_one_value_undoes_as_one_step() {
        let start = Instant::now();
        let mut history = History::default();
        history.push(step("lighting", 1, 2, start));
        history.push(step("lighting", 2, 3, start + Duration::from_millis(100)));
        history.push(step("lighting", 3, 4, start + Duration::from_millis(200)));
        assert_eq!(spans(&history), [(vec![1], vec![4])]);
    }

    #[test]
    fn changes_to_other_values_or_after_a_pause_stay_separate_steps() {
        let start = Instant::now();
        let mut history = History::default();
        history.push(step("lighting", 1, 2, start));
        history.push(step("keymap", 7, 8, start));
        history.push(step("keymap", 5, 6, start));
        history.push(step("keymap", 6, 9, start + MERGE_WINDOW));
        assert_eq!(
            spans(&history),
            [
                (vec![1], vec![2]),
                (vec![7], vec![8]),
                (vec![5], vec![6]),
                (vec![6], vec![9]),
            ]
        );
    }

    #[test]
    fn a_chain_that_returns_to_its_start_leaves_no_step() {
        let start = Instant::now();
        let mut history = History::default();
        history.push(step("lighting", 1, 2, start));
        history.push(step("lighting", 2, 1, start));
        assert!(!history.can_undo());
    }

    #[test]
    fn undo_moves_a_step_to_redo_and_a_new_change_clears_redo() {
        let start = Instant::now();
        let mut history = History::default();
        history.push(step("lighting", 1, 2, start));
        assert!(history.undo().is_some());
        assert!(history.can_redo());
        assert!(history.redo().is_some());
        assert!(history.can_undo());
        history.undo();
        history.push(step("keymap", 3, 4, start + MERGE_WINDOW));
        assert!(!history.can_redo());
    }
}
