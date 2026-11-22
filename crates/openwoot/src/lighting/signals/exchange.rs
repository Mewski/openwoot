use core::cell::RefCell;

use embassy_sync::blocking_mutex::{Mutex, raw::RawMutex};
use openwoot_protocol::lightbar::{HostSignal, HostSignalIds, HostSignals};

use crate::lighting::hooks::Signals;

pub struct HostExchange<M: RawMutex> {
    state: Mutex<M, RefCell<Exchanged>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Exchanged {
    ids: HostSignalIds,
    pending: HostSignals,
    activated: bool,
}

impl<M: RawMutex> HostExchange<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(Exchanged {
                ids: HostSignalIds::new(),
                pending: HostSignals::new(),
                activated: false,
            })),
        }
    }

    #[must_use]
    pub fn ids(&self) -> HostSignalIds {
        self.state.lock(|state| {
            let mut state = state.borrow_mut();
            state.activated = true;
            state.ids.clone()
        })
    }

    pub fn set(&self, values: &[HostSignal]) {
        self.state.lock(|state| {
            let mut state = state.borrow_mut();
            state.activated = true;
            for value in values {
                if let Some(pending) = state
                    .pending
                    .iter_mut()
                    .find(|pending| pending.id == value.id)
                {
                    pending.value = value.value;
                } else {
                    let _ = state.pending.push(*value);
                }
            }
        });
    }

    pub fn sync(&self, signals: &mut impl Signals) {
        let (pending, activated) = self.state.lock(|state| {
            let mut state = state.borrow_mut();
            (
                core::mem::take(&mut state.pending),
                core::mem::take(&mut state.activated),
            )
        });
        if activated {
            signals.activate_host_signals();
        }
        if !pending.is_empty() {
            signals.set_host_signals(&pending);
        }
        let ids = signals.host_signal_ids();
        self.state.lock(|state| state.borrow_mut().ids = ids);
    }
}

impl<M: RawMutex> Default for HostExchange<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> core::fmt::Debug for HostExchange<M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.state.lock(|state| {
            formatter
                .debug_struct("HostExchange")
                .field("ids", &state.borrow().ids)
                .finish_non_exhaustive()
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        activated: bool,
        values: Vec<HostSignal>,
    }

    impl Signals for Recorder {
        fn activate_host_signals(&mut self) {
            self.activated = true;
        }

        fn set_host_signals(&mut self, values: &[HostSignal]) {
            self.values.extend_from_slice(values);
        }

        fn host_signal_ids(&self) -> HostSignalIds {
            [7, 3].into_iter().collect()
        }
    }

    #[test]
    fn values_wait_for_the_lighting_task_and_the_last_value_of_an_id_wins() {
        let exchange = HostExchange::<CriticalSectionRawMutex>::new();
        assert_eq!(exchange.ids().as_slice(), &[] as &[u32]);
        exchange.set(&[
            HostSignal { id: 7, value: 1.0 },
            HostSignal { id: 3, value: 2.0 },
        ]);
        exchange.set(&[HostSignal { id: 7, value: 4.0 }]);
        let mut recorder = Recorder::default();
        exchange.sync(&mut recorder);
        assert!(recorder.activated);
        assert_eq!(
            recorder.values,
            [
                HostSignal { id: 7, value: 4.0 },
                HostSignal { id: 3, value: 2.0 }
            ]
        );
        assert_eq!(exchange.ids().as_slice(), &[7, 3]);
        let mut idle = Recorder::default();
        exchange.sync(&mut idle);
        assert!(idle.activated);
        assert_eq!(idle.values, []);
    }
}
