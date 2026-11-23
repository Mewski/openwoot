use core::cell::RefCell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};
use heapless::Deque;

use crate::{
    hid::{ConsumerReport, KeyStateReport, MouseReport, SystemReport},
    keys::{GamepadReport, Keyboard, Output},
};

const EDGE_BACKLOG: usize = 4;
const LATEST_ONLY: usize = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    pub keyboard: Option<Keyboard>,
    pub system: Option<SystemReport>,
    pub consumer: Option<ConsumerReport>,
    pub mouse: Option<MouseReport>,
    pub key_states: Option<KeyStateReport>,
    pub gamepad: Option<GamepadReport>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlsReport {
    System(SystemReport),
    Consumer(ConsumerReport),
    Mouse(MouseReport),
}

struct Backlog<T, const DEPTH: usize> {
    offered: Option<T>,
    waiting: Deque<T, DEPTH>,
}

struct Queue {
    keyboard: Backlog<Keyboard, EDGE_BACKLOG>,
    system: Backlog<SystemReport, EDGE_BACKLOG>,
    consumer: Backlog<ConsumerReport, EDGE_BACKLOG>,
    mouse: Backlog<MouseReport, EDGE_BACKLOG>,
    key_states: Backlog<KeyStateReport, LATEST_ONLY>,
    gamepad: Backlog<GamepadReport, LATEST_ONLY>,
}

pub struct KeyReports<M: RawMutex> {
    queue: Mutex<M, RefCell<Queue>>,
    keyboard_ready: Signal<M, ()>,
    controls_ready: Signal<M, ()>,
    key_states_ready: Signal<M, ()>,
    gamepad_ready: Signal<M, ()>,
    wake_up: Signal<M, ()>,
}

impl<T: Copy + PartialEq, const DEPTH: usize> Backlog<T, DEPTH> {
    const fn new() -> Self {
        Self {
            offered: None,
            waiting: Deque::new(),
        }
    }

    fn offer(&mut self, report: T) -> bool {
        if self.offered == Some(report) {
            return false;
        }
        self.offered = Some(report);
        if let Err(report) = self.waiting.push_back(report)
            && let Some(newest) = self.waiting.back_mut()
        {
            *newest = report;
        }
        true
    }

    fn pop(&mut self) -> Option<T> {
        self.waiting.pop_front()
    }

    fn take_newest(&mut self) -> Option<T> {
        let newest = self.waiting.back().copied();
        self.waiting.clear();
        newest
    }

    fn restart(&mut self) {
        self.offered = None;
        self.waiting.clear();
    }
}

impl Queue {
    const fn new() -> Self {
        Self {
            keyboard: Backlog::new(),
            system: Backlog::new(),
            consumer: Backlog::new(),
            mouse: Backlog::new(),
            key_states: Backlog::new(),
            gamepad: Backlog::new(),
        }
    }

    fn pop_controls(&mut self) -> Option<ControlsReport> {
        self.system
            .pop()
            .map(ControlsReport::System)
            .or_else(|| self.consumer.pop().map(ControlsReport::Consumer))
            .or_else(|| self.mouse.pop().map(ControlsReport::Mouse))
    }

    fn take_newest(&mut self) -> Pending {
        Pending {
            keyboard: self.keyboard.take_newest(),
            system: self.system.take_newest(),
            consumer: self.consumer.take_newest(),
            mouse: self.mouse.take_newest(),
            key_states: self.key_states.take_newest(),
            gamepad: self.gamepad.take_newest(),
        }
    }

    fn restart(&mut self) {
        self.keyboard.restart();
        self.system.restart();
        self.consumer.restart();
        self.mouse.restart();
        self.key_states.restart();
        self.gamepad.restart();
    }
}

impl<M: RawMutex> KeyReports<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(RefCell::new(Queue::new())),
            keyboard_ready: Signal::new(),
            controls_ready: Signal::new(),
            key_states_ready: Signal::new(),
            gamepad_ready: Signal::new(),
            wake_up: Signal::new(),
        }
    }

    pub fn publish(&self, output: &Output) -> bool {
        let [keyboard, controls, key_states, gamepad] = self.queue.lock(|queue| {
            let Ok(mut queue) = queue.try_borrow_mut() else {
                return [false; 4];
            };
            let keyboard = queue.keyboard.offer(output.keyboard());
            let mut controls = queue.system.offer(*output.system());
            controls |= queue.consumer.offer(*output.consumer());
            controls |= queue.mouse.offer(*output.mouse());
            let key_states = queue.key_states.offer(*output.key_states());
            let gamepad = match output.gamepad() {
                Some(report) => queue.gamepad.offer(*report),
                None => false,
            };
            [keyboard, controls, key_states, gamepad]
        });
        if keyboard {
            self.keyboard_ready.signal(());
        }
        if controls {
            self.controls_ready.signal(());
        }
        if key_states {
            self.key_states_ready.signal(());
        }
        if gamepad {
            self.gamepad_ready.signal(());
        }
        keyboard || controls || key_states || gamepad
    }

    pub fn resend(&self) {
        self.queue.lock(|queue| {
            if let Ok(mut queue) = queue.try_borrow_mut() {
                queue.restart();
            }
        });
    }

    #[must_use]
    pub fn take(&self) -> Pending {
        self.queue.lock(|queue| {
            queue
                .try_borrow_mut()
                .map_or_else(|_| Pending::default(), |mut queue| queue.take_newest())
        })
    }

    pub fn request_wake_up(&self) {
        self.wake_up.signal(());
    }

    pub(super) fn forget_wake_up(&self) {
        self.wake_up.reset();
    }

    pub(super) async fn wake_up_requested(&self) {
        self.wake_up.wait().await;
    }

    pub(super) async fn keyboard(&self) -> Keyboard {
        loop {
            if let Some(report) = self.pop(|queue| queue.keyboard.pop()) {
                return report;
            }
            self.keyboard_ready.wait().await;
        }
    }

    pub(super) async fn controls(&self) -> ControlsReport {
        loop {
            if let Some(report) = self.pop(Queue::pop_controls) {
                return report;
            }
            self.controls_ready.wait().await;
        }
    }

    pub(super) async fn key_states(&self) -> KeyStateReport {
        loop {
            if let Some(report) = self.pop(|queue| queue.key_states.pop()) {
                return report;
            }
            self.key_states_ready.wait().await;
        }
    }

    pub(super) async fn gamepad(&self) -> GamepadReport {
        loop {
            if let Some(report) = self.pop(|queue| queue.gamepad.pop()) {
                return report;
            }
            self.gamepad_ready.wait().await;
        }
    }

    fn pop<R>(&self, pop: impl FnOnce(&mut Queue) -> Option<R>) -> Option<R> {
        self.queue.lock(|queue| {
            let mut queue = queue.try_borrow_mut().ok()?;
            let queue: &mut Queue = &mut queue;
            pop(queue)
        })
    }
}

impl<M: RawMutex> Default for KeyReports<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> core::fmt::Debug for KeyReports<M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("KeyReports").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backlog_hands_out_every_change_in_order_and_skips_repeats() {
        let mut backlog = Backlog::<u8, 4>::new();
        assert!(backlog.offer(1));
        assert!(!backlog.offer(1));
        assert!(backlog.offer(0));
        assert!(backlog.offer(1));
        assert_eq!(backlog.pop(), Some(1));
        assert_eq!(backlog.pop(), Some(0));
        assert_eq!(backlog.pop(), Some(1));
        assert_eq!(backlog.pop(), None);
        assert!(!backlog.offer(1));
    }

    #[test]
    fn a_full_backlog_replaces_its_newest_entry_so_the_last_state_is_never_lost() {
        let mut backlog = Backlog::<u8, 2>::new();
        assert!(backlog.offer(1));
        assert!(backlog.offer(2));
        assert!(backlog.offer(3));
        assert_eq!(backlog.pop(), Some(1));
        assert_eq!(backlog.pop(), Some(3));
        assert_eq!(backlog.pop(), None);
    }

    #[test]
    fn a_restart_forgets_what_waits_and_offers_the_same_state_again() {
        let mut backlog = Backlog::<u8, 2>::new();
        assert!(backlog.offer(7));
        backlog.restart();
        assert_eq!(backlog.pop(), None);
        assert!(backlog.offer(7));
        assert_eq!(backlog.take_newest(), Some(7));
        assert_eq!(backlog.pop(), None);
    }
}
