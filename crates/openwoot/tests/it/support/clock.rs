use std::sync::{Mutex, MutexGuard, PoisonError};

static CLOCK: Mutex<()> = Mutex::new(());

pub(crate) fn hold_clock() -> MutexGuard<'static, ()> {
    CLOCK.lock().unwrap_or_else(PoisonError::into_inner)
}
