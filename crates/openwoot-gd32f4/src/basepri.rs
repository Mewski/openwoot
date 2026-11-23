use crate::interrupt::Priority;

pub(crate) const MASK: u8 = Priority::HIGHEST.to_nvic();

const _: () = assert!(Priority::SCAN.to_nvic() < MASK);

#[cfg(target_os = "none")]
mod implementation {
    use cortex_m::register::{basepri, basepri_max};

    use super::MASK;

    struct BasepriCriticalSection;

    critical_section::set_impl!(BasepriCriticalSection);

    // SAFETY: BASEPRI masks every interrupt of priority `MASK` or lower (`executor::run` selects
    // the priority grouping without sub-priorities first); the single core runs nothing else, so
    // code between `acquire` and `release` excludes every context that uses a critical section.
    // The scan interrupt (priority 0) is never masked and takes no critical section: the HAL's
    // scan path takes none, and a `scan::Conversion` hook must take none either.
    unsafe impl critical_section::Impl for BasepriCriticalSection {
        // SAFETY: callers pair it with `release`, as the `critical-section` contract demands.
        unsafe fn acquire() -> u8 {
            let previous = basepri::read();
            basepri_max::write(MASK);
            previous
        }

        // SAFETY: callers pass the value of the matching `acquire`.
        unsafe fn release(restore_state: u8) {
            // SAFETY: `restore_state` is the BASEPRI value that the matching `acquire` found, so
            // the nesting level of critical sections is restored.
            unsafe { basepri::write(restore_state) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mask_covers_every_level_but_the_scan() {
        for level in 1..16 {
            let priority = Priority::new(level).unwrap();
            assert!(priority.to_nvic() >= MASK, "level {level} stays unmasked");
        }
        assert!(Priority::SCAN.to_nvic() < MASK);
        assert_eq!(MASK, 0x10);
    }
}
