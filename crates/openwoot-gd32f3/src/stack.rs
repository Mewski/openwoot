use openwoot::diagnostics::Stack;

#[must_use]
pub(crate) fn usage() -> Stack {
    #[cfg(target_os = "none")]
    {
        painted::usage()
    }
    #[cfg(not(target_os = "none"))]
    {
        Stack::default()
    }
}

#[cfg(target_os = "none")]
mod painted {
    use core::ptr;

    use cortex_m_rt::STACK_PAINT_VALUE;
    use openwoot::diagnostics::Stack;

    const WORD: usize = 4;

    // SAFETY: `cortex-m-rt`'s linker script defines both symbols as the lowest and the highest
    // address of the stack; only their addresses are taken, never their contents.
    unsafe extern "C" {
        safe static _stack_end: u32;
        safe static _stack_start: u32;
    }

    pub(super) fn usage() -> Stack {
        let lowest = (&raw const _stack_end).addr();
        let highest = (&raw const _stack_start).addr();
        let untouched = (lowest..highest)
            .step_by(WORD)
            .take_while(|&address| is_painted(address))
            .count();
        let size = highest - lowest;
        Stack {
            size: u32::try_from(size).unwrap_or(u32::MAX),
            peak: u32::try_from(size - untouched * WORD).unwrap_or(u32::MAX),
        }
    }

    fn is_painted(address: usize) -> bool {
        let word = ptr::with_exposed_provenance::<u32>(address);
        // SAFETY: the address is a word inside the stack region, which `cortex-m-rt` painted at
        // reset; reading it disturbs no frame that uses it.
        unsafe { ptr::read_volatile(word) == STACK_PAINT_VALUE }
    }
}
