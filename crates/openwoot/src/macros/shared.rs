use core::cell::RefCell;

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};
use openwoot_protocol::{
    device::Layout,
    macros::{MAX_ENCODED_LEN, Mode, Step},
    wire,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub mode: Mode,
    pub layout: Layout,
    pub steps: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loaded {
    Pending,
    Ready(Header),
    Missing,
}

pub struct Shared<M: RawMutex> {
    wanted: Signal<M, ()>,
    slot: Mutex<M, RefCell<Slot>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Load {
    Idle,
    Wanted(u8),
    Ready(u8, Header),
    Missing(u8),
}

struct Slot {
    load: Load,
    len: usize,
    steps: [u8; MAX_ENCODED_LEN],
}

impl<M: RawMutex> Shared<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            wanted: Signal::new(),
            slot: Mutex::new(RefCell::new(Slot {
                load: Load::Idle,
                len: 0,
                steps: [0; MAX_ENCODED_LEN],
            })),
        }
    }

    pub fn want(&self, index: u8) {
        self.update(|slot| slot.load = Load::Wanted(index));
        self.wanted.signal(());
    }

    #[must_use]
    pub fn loaded(&self, index: u8) -> Loaded {
        let load = self
            .slot
            .lock(|slot| slot.try_borrow().map(|slot| slot.load));
        match load {
            Ok(Load::Ready(loaded, header)) if loaded == index => Loaded::Ready(header),
            Ok(Load::Missing(missing)) if missing == index => Loaded::Missing,
            _ => Loaded::Pending,
        }
    }

    #[must_use]
    pub fn step(&self, index: u8, offset: usize) -> Option<(Step, usize)> {
        self.slot.lock(|slot| {
            let slot = slot.try_borrow().ok()?;
            if !matches!(slot.load, Load::Ready(loaded, _) if loaded == index) {
                return None;
            }
            let steps = slot.steps.get(..slot.len)?;
            let (step, rest) = wire::take::<Step>(steps.get(offset..)?)?;
            Some((step, steps.len() - rest.len()))
        })
    }

    pub async fn wanted(&self) -> u8 {
        loop {
            self.wanted.wait().await;
            let load = self
                .slot
                .lock(|slot| slot.try_borrow().map(|slot| slot.load));
            if let Ok(Load::Wanted(index)) = load {
                return index;
            }
        }
    }

    pub fn fill(&self, index: u8, header: Header, steps: &[u8]) {
        self.update(|slot| {
            if slot.load != Load::Wanted(index) {
                return;
            }
            slot.load = match slot.steps.get_mut(..steps.len()) {
                Some(buffer) => {
                    buffer.copy_from_slice(steps);
                    slot.len = steps.len();
                    Load::Ready(index, header)
                }
                None => Load::Missing(index),
            };
        });
    }

    pub fn missing(&self, index: u8) {
        self.update(|slot| {
            if slot.load == Load::Wanted(index) {
                slot.load = Load::Missing(index);
            }
        });
    }

    fn update(&self, update: impl FnOnce(&mut Slot)) {
        self.slot.lock(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                update(&mut slot);
            }
        });
    }
}

impl<M: RawMutex> Default for Shared<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> core::fmt::Debug for Shared<M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("Shared").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use embassy_futures::{block_on, poll_once};
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;
    use openwoot_protocol::keys::Action;

    use super::*;

    const HEADER: Header = Header {
        mode: Mode::Once,
        layout: Layout::Iso,
        steps: 2,
    };
    const STEPS: [u8; 5] = [2, 1, 0x04, 3, 0x0A];

    #[test]
    fn a_wanted_macro_reaches_the_keys_once_thread_mode_filled_it() {
        let shared = Shared::<NoopRawMutex>::new();
        assert_eq!(shared.loaded(3), Loaded::Pending);
        shared.want(3);
        assert_eq!(block_on(shared.wanted()), 3);
        assert_eq!(shared.loaded(3), Loaded::Pending);
        assert_eq!(shared.step(3, 0), None);
        shared.fill(3, HEADER, &STEPS);
        assert_eq!(shared.loaded(3), Loaded::Ready(HEADER));
        assert_eq!(shared.loaded(4), Loaded::Pending);
        assert_eq!(
            shared.step(3, 0),
            Some((Step::Tap(Action::Keyboard(0x04)), 3))
        );
        assert_eq!(shared.step(3, 3), Some((Step::Delay(10), 5)));
        assert_eq!(shared.step(3, 5), None);
        assert_eq!(shared.step(4, 0), None);
    }

    #[test]
    fn a_fill_for_a_macro_no_longer_wanted_changes_nothing() {
        let shared = Shared::<NoopRawMutex>::new();
        shared.want(3);
        shared.want(5);
        shared.fill(3, HEADER, &STEPS);
        assert_eq!(shared.loaded(3), Loaded::Pending);
        shared.missing(3);
        assert_eq!(shared.loaded(5), Loaded::Pending);
        shared.missing(5);
        assert_eq!(shared.loaded(5), Loaded::Missing);
        shared.fill(5, HEADER, &STEPS);
        assert_eq!(shared.loaded(5), Loaded::Missing);
    }

    #[test]
    fn thread_mode_waits_while_nothing_is_wanted() {
        let shared = Shared::<NoopRawMutex>::new();
        let mut wanted = core::pin::pin!(shared.wanted());
        assert_eq!(poll_once(wanted.as_mut()), core::task::Poll::Pending);
        shared.want(7);
        assert_eq!(poll_once(wanted.as_mut()), core::task::Poll::Ready(7));
    }

    #[test]
    fn steps_longer_than_the_buffer_load_as_missing() {
        let shared = Shared::<NoopRawMutex>::new();
        shared.want(1);
        shared.fill(1, HEADER, &[0; MAX_ENCODED_LEN + 1]);
        assert_eq!(shared.loaded(1), Loaded::Missing);
    }
}
