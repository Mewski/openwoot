use embassy_usb_synopsys_otg::{StateStorage, on_interrupt};

use super::registers::registers;
use crate::interrupt::{Handler, typelevel};

pub(super) const ENDPOINT_COUNT: usize = 6;

pub(super) static STATE: StateStorage<ENDPOINT_COUNT> = StateStorage::new();

#[derive(Debug)]
pub struct InterruptHandler;

impl Handler<typelevel::USBHS> for InterruptHandler {
    // SAFETY: the handler runs only in the USBHS interrupt, as `Handler` requires.
    unsafe fn on_interrupt() {
        // SAFETY: the register block is the controller the driver owns, and `STATE` is the state
        // the driver was built with.
        unsafe { on_interrupt(registers(), &STATE.as_state()) };
    }
}
