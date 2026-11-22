#![allow(
    clippy::inline_always,
    reason = "the hook is inlined into the scan interrupt as the C HAL's always_inline helpers"
)]

use core::marker::PhantomData;

use crate::interrupt::{Handler, typelevel};

pub trait Conversion {
    fn on_conversion();
}

#[derive(Debug)]
pub struct InterruptHandler<Hook>(PhantomData<Hook>);

impl<Hook: Conversion> Handler<typelevel::ADC0_1> for InterruptHandler<Hook> {
    // SAFETY: the handler runs only in the ADC0_1 interrupt, as `Handler` requires; the hook it
    // calls is safe code.
    #[inline(always)]
    unsafe fn on_interrupt() {
        Hook::on_conversion();
    }
}
