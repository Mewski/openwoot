use core::cell::RefCell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};

use crate::{
    hid::{ConsumerReport, KeyStateReport, MouseReport, SystemReport},
    keys::{GamepadReport, Keyboard, Output},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    pub keyboard: Option<Keyboard>,
    pub system: Option<SystemReport>,
    pub consumer: Option<ConsumerReport>,
    pub mouse: Option<MouseReport>,
    pub key_states: Option<KeyStateReport>,
    pub gamepad: Option<GamepadReport>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Queue {
    pending: Pending,
    sent: Pending,
}

pub struct KeyReports<M: RawMutex> {
    queue: Mutex<M, RefCell<Queue>>,
    ready: Signal<M, ()>,
}

impl<M: RawMutex> KeyReports<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(RefCell::new(Queue {
                pending: Pending {
                    keyboard: None,
                    system: None,
                    consumer: None,
                    mouse: None,
                    key_states: None,
                    gamepad: None,
                },
                sent: Pending {
                    keyboard: None,
                    system: None,
                    consumer: None,
                    mouse: None,
                    key_states: None,
                    gamepad: None,
                },
            })),
            ready: Signal::new(),
        }
    }

    pub fn publish(&self, output: &Output) -> bool {
        let published = self.queue.lock(|queue| {
            let Ok(mut queue) = queue.try_borrow_mut() else {
                return false;
            };
            let Queue { pending, sent } = &mut *queue;
            let mut published = false;
            published |= offer(&mut pending.keyboard, &mut sent.keyboard, output.keyboard());
            published |= offer(&mut pending.system, &mut sent.system, *output.system());
            published |= offer(
                &mut pending.consumer,
                &mut sent.consumer,
                *output.consumer(),
            );
            published |= offer(&mut pending.mouse, &mut sent.mouse, *output.mouse());
            published |= offer(
                &mut pending.key_states,
                &mut sent.key_states,
                *output.key_states(),
            );
            if let Some(gamepad) = output.gamepad() {
                published |= offer(&mut pending.gamepad, &mut sent.gamepad, *gamepad);
            }
            published
        });
        if published {
            self.ready.signal(());
        }
        published
    }

    pub fn resend(&self) {
        self.queue.lock(|queue| {
            if let Ok(mut queue) = queue.try_borrow_mut() {
                queue.sent = Pending::default();
            }
        });
    }

    #[must_use]
    pub fn take(&self) -> Pending {
        self.queue.lock(|queue| {
            queue.try_borrow_mut().map_or_else(
                |_| Pending::default(),
                |mut queue| core::mem::take(&mut queue.pending),
            )
        })
    }

    pub async fn ready(&self) -> Pending {
        self.ready.wait().await;
        self.take()
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

fn offer<T: Copy + PartialEq>(pending: &mut Option<T>, sent: &mut Option<T>, report: T) -> bool {
    if *sent == Some(report) {
        return false;
    }
    *sent = Some(report);
    *pending = Some(report);
    true
}
