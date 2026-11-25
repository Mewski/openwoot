pub trait Recheck: Default {
    fn reconverts(&self, input: u8) -> bool;

    fn reconvert_single(&mut self, input: u8, value: u16, filtered: u16) -> bool;

    fn defer(&mut self, input: u8, value: u16, filtered: u16) -> bool;

    fn reconvert_deferred(&mut self) -> bool;

    fn is_second_conversion(&self) -> bool;

    fn is_deferred(&self, input: u8) -> bool;

    fn reset(&mut self);
}

impl Recheck for () {
    #[inline(always)]
    fn reconverts(&self, _input: u8) -> bool {
        false
    }

    #[inline(always)]
    fn reconvert_single(&mut self, _input: u8, _value: u16, _filtered: u16) -> bool {
        false
    }

    #[inline(always)]
    fn defer(&mut self, _input: u8, _value: u16, _filtered: u16) -> bool {
        false
    }

    #[inline(always)]
    fn reconvert_deferred(&mut self) -> bool {
        false
    }

    #[inline(always)]
    fn is_second_conversion(&self) -> bool {
        false
    }

    #[inline(always)]
    fn is_deferred(&self, _input: u8) -> bool {
        false
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecheckFrom<const FIRST_INPUT: u8> {
    second_single: bool,
    second_parallel: bool,
    deferred: u16,
}

impl<const FIRST_INPUT: u8> Recheck for RecheckFrom<FIRST_INPUT> {
    #[inline(always)]
    fn reconverts(&self, input: u8) -> bool {
        input >= FIRST_INPUT
    }

    #[inline(always)]
    fn reconvert_single(&mut self, input: u8, value: u16, filtered: u16) -> bool {
        if input >= FIRST_INPUT && !self.second_single && value != filtered {
            self.second_single = true;
            return true;
        }
        self.second_single = false;
        false
    }

    #[inline(always)]
    fn defer(&mut self, input: u8, value: u16, filtered: u16) -> bool {
        if input >= FIRST_INPUT && value != filtered {
            self.deferred |= 1 << (input & 15);
            return true;
        }
        false
    }

    #[inline(always)]
    fn reconvert_deferred(&mut self) -> bool {
        if self.deferred != 0 {
            self.second_parallel = true;
            return true;
        }
        false
    }

    #[inline(always)]
    fn is_second_conversion(&self) -> bool {
        self.second_parallel
    }

    #[inline(always)]
    fn is_deferred(&self, input: u8) -> bool {
        self.deferred & (1 << (input & 15)) != 0
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.deferred = 0;
        self.second_parallel = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_single_reading_is_converted_once_more() {
        let mut recheck = RecheckFrom::<3>::default();
        assert!(!recheck.reconvert_single(2, 10, 0));
        assert!(recheck.reconvert_single(3, 10, 0));
        assert!(!recheck.reconvert_single(3, 10, 0));
        assert!(!recheck.reconvert_single(3, 10, 10));
    }

    #[test]
    fn changed_parallel_readings_wait_for_one_second_conversion() {
        let mut recheck = RecheckFrom::<3>::default();
        assert!(!recheck.defer(1, 5, 0));
        assert!(!recheck.defer(3, 5, 5));
        assert!(recheck.defer(4, 5, 0));
        assert!(recheck.reconvert_deferred());
        assert!(recheck.is_second_conversion());
        assert!(recheck.is_deferred(4) && !recheck.is_deferred(3));
        recheck.reset();
        assert!(!recheck.is_second_conversion() && !recheck.reconvert_deferred());
    }
}
