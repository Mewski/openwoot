use core::sync::atomic::{AtomicU8, Ordering};

use openwoot_protocol::diagnostics::{Bus, BusError, BusErrors};

use super::Counter;

const NO_ERROR: u8 = 0;

#[derive(Debug, Default)]
pub struct Transfers {
    completed: Counter,
    timeout: Counter,
    bus: Counter,
    arbitration_loss: Counter,
    no_acknowledge: Counter,
    overrun: Counter,
    dma: Counter,
    other: Counter,
    last_error: AtomicU8,
}

impl Transfers {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            completed: Counter::new(),
            timeout: Counter::new(),
            bus: Counter::new(),
            arbitration_loss: Counter::new(),
            no_acknowledge: Counter::new(),
            overrun: Counter::new(),
            dma: Counter::new(),
            other: Counter::new(),
            last_error: AtomicU8::new(NO_ERROR),
        }
    }

    pub fn record<E: Copy + Into<BusError>>(&self, result: &Result<(), E>) {
        match result {
            Ok(()) => self.completed.increment(),
            Err(error) => {
                let kind = (*error).into();
                self.errors(kind).increment();
                self.last_error.store(code(kind), Ordering::Relaxed);
            }
        }
    }

    #[must_use]
    pub fn report(&self) -> Bus {
        Bus {
            completed: self.completed.value(),
            errors: BusErrors {
                timeout: self.timeout.value(),
                bus: self.bus.value(),
                arbitration_loss: self.arbitration_loss.value(),
                no_acknowledge: self.no_acknowledge.value(),
                overrun: self.overrun.value(),
                dma: self.dma.value(),
                other: self.other.value(),
            },
            last_error: kind(self.last_error.load(Ordering::Relaxed)),
        }
    }

    const fn errors(&self, kind: BusError) -> &Counter {
        match kind {
            BusError::Timeout => &self.timeout,
            BusError::Bus => &self.bus,
            BusError::ArbitrationLoss => &self.arbitration_loss,
            BusError::NoAcknowledge => &self.no_acknowledge,
            BusError::Overrun => &self.overrun,
            BusError::Dma => &self.dma,
            BusError::Other => &self.other,
        }
    }
}

fn code(kind: BusError) -> u8 {
    (1..)
        .zip(BusErrors::KINDS)
        .find_map(|(code, known)| (known == kind).then_some(code))
        .unwrap_or(NO_ERROR)
}

fn kind(code: u8) -> Option<BusError> {
    BusErrors::KINDS
        .get(usize::from(code.checked_sub(1)?))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfers_count_by_kind_and_keep_the_last_error() {
        let transfers = Transfers::new();
        assert_eq!(transfers.report(), Bus::default());
        transfers.record::<BusError>(&Ok(()));
        transfers.record(&Err(BusError::NoAcknowledge));
        transfers.record(&Err(BusError::Timeout));
        transfers.record(&Err(BusError::NoAcknowledge));
        transfers.record::<BusError>(&Ok(()));
        let report = transfers.report();
        assert_eq!(report.completed, 2);
        assert_eq!(report.errors.no_acknowledge, 2);
        assert_eq!(report.errors.timeout, 1);
        assert_eq!(report.errors.total(), 3);
        assert_eq!(report.last_error, Some(BusError::NoAcknowledge));
    }

    #[test]
    fn every_error_kind_survives_its_stored_code() {
        for known in BusErrors::KINDS {
            assert_eq!(kind(code(known)), Some(known));
        }
        assert_eq!(kind(NO_ERROR), None);
    }
}
