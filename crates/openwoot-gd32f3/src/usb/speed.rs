use core::sync::atomic::{AtomicBool, Ordering};

use embassy_usb_driver::Speed;

static ENUMERATED: AtomicBool = AtomicBool::new(false);

#[must_use]
pub fn negotiated_speed() -> Option<Speed> {
    ENUMERATED.load(Ordering::Acquire).then_some(Speed::Full)
}

pub(super) fn record() {
    ENUMERATED.store(true, Ordering::Release);
}
