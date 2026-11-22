use crate::scan::Mode;

const HOLD_MS: u32 = 5000;
const PASS_PAUSE_MS: u32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Batching {
    Addresses,
    InputPairs,
}

impl Batching {
    #[must_use]
    pub const fn count(self, inputs: usize, addresses: usize) -> usize {
        match self {
            Self::Addresses => addresses,
            Self::InputPairs => inputs.div_ceil(2),
        }
    }

    pub fn members(
        self,
        batch: u8,
        inputs: usize,
        addresses: usize,
    ) -> impl Iterator<Item = (u8, u8)> {
        let batch = usize::from(batch);
        let (pairs, columns) = match self {
            Self::InputPairs => (2 * addresses, 0),
            Self::Addresses => (0, inputs),
        };
        let pair_members = (0..pairs)
            .map(move |member| (2 * batch + member / addresses, member % addresses))
            .take_while(move |&(input, _)| input < inputs);
        let column_members = (0..columns).map(move |input| (input, batch));
        pair_members
            .chain(column_members)
            .map(|(input, address)| (input as u8, address as u8))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Wait,
    Continue,
    StartBatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Advance {
    NextBatch,
    PassComplete,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags of the recalibration schedule"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recalibration {
    batching: Batching,
    batch: u8,
    hold: bool,
    hold_stamp_ms: u32,
    running: bool,
    pass_done: bool,
    pass_done_stamp_ms: u32,
    done_single: bool,
    done_parallel: bool,
}

impl Default for Recalibration {
    fn default() -> Self {
        Self::new()
    }
}

impl Recalibration {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            batching: Batching::InputPairs,
            batch: 0,
            hold: false,
            hold_stamp_ms: 0,
            running: false,
            pass_done: false,
            pass_done_stamp_ms: 0,
            done_single: false,
            done_parallel: false,
        }
    }

    #[must_use]
    pub const fn batching(&self) -> Batching {
        self.batching
    }

    #[must_use]
    pub const fn batch(&self) -> u8 {
        self.batch
    }

    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    #[must_use]
    pub const fn is_pausing(&self) -> bool {
        self.pass_done
    }

    #[must_use]
    pub const fn is_held(&self) -> bool {
        self.hold
    }

    #[must_use]
    pub const fn has_completed(&self, mode: Mode) -> bool {
        match mode {
            Mode::Single => self.done_single,
            Mode::Parallel => self.done_parallel,
        }
    }

    pub(crate) fn stop(&mut self) -> bool {
        core::mem::replace(&mut self.running, false)
    }

    pub(crate) fn restart(&mut self, mode: Mode) {
        self.batch = 0;
        self.pass_done = false;
        self.hold = false;
        self.batching = match mode {
            Mode::Single if self.done_single && self.done_parallel => Batching::Addresses,
            Mode::Parallel if self.done_parallel => Batching::Addresses,
            Mode::Single | Mode::Parallel => Batching::InputPairs,
        };
    }

    pub(crate) fn hold(&mut self, now_ms: u32) {
        self.hold_stamp_ms = now_ms;
        self.hold = true;
    }

    pub(crate) fn decide(&mut self, now_ms: u32) -> Decision {
        if self.running {
            return Decision::Continue;
        }
        if self.hold {
            if now_ms.wrapping_sub(self.hold_stamp_ms) < HOLD_MS {
                if self.pass_done && now_ms.wrapping_sub(self.pass_done_stamp_ms) >= PASS_PAUSE_MS {
                    self.pass_done = false;
                }
                return Decision::Wait;
            }
            self.hold = false;
        }
        if self.pass_done {
            if now_ms.wrapping_sub(self.pass_done_stamp_ms) < PASS_PAUSE_MS {
                return Decision::Wait;
            }
            self.pass_done = false;
        }
        self.hold_stamp_ms = now_ms;
        self.running = true;
        Decision::StartBatch
    }

    pub(crate) fn advance(
        &mut self,
        mode: Mode,
        now_ms: u32,
        inputs: usize,
        addresses: usize,
    ) -> Advance {
        self.batch = self.batch.wrapping_add(1);
        if usize::from(self.batch) < self.batching.count(inputs, addresses) {
            return Advance::NextBatch;
        }
        self.batch = 0;
        self.running = false;
        self.pass_done_stamp_ms = now_ms;
        self.pass_done = true;
        match mode {
            Mode::Single => self.done_single = true,
            Mode::Parallel => self.done_parallel = true,
        }
        self.batching = Batching::Addresses;
        Advance::PassComplete
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    #[test]
    fn input_pairs_cover_both_inputs_of_a_pair_address_by_address() {
        let members: Vec<_> = Batching::InputPairs.members(1, 5, 3).collect();
        assert_eq!(members, [(2, 0), (2, 1), (2, 2), (3, 0), (3, 1), (3, 2)]);
        let last: Vec<_> = Batching::InputPairs.members(2, 5, 3).collect();
        assert_eq!(last, [(4, 0), (4, 1), (4, 2)]);
        assert_eq!(Batching::InputPairs.count(5, 3), 3);
    }

    #[test]
    fn an_address_batch_covers_every_input_at_that_address() {
        let members: Vec<_> = Batching::Addresses.members(7, 4, 16).collect();
        assert_eq!(members, [(0, 7), (1, 7), (2, 7), (3, 7)]);
        assert_eq!(Batching::Addresses.count(4, 16), 16);
    }

    #[test]
    fn key_activity_holds_recalibration_for_five_seconds() {
        let mut recalibration = Recalibration::new();
        recalibration.hold(1000);
        assert_eq!(recalibration.decide(5999), Decision::Wait);
        assert_eq!(recalibration.decide(6000), Decision::StartBatch);
        assert_eq!(recalibration.decide(6001), Decision::Continue);
    }

    #[test]
    fn a_complete_pass_pauses_twenty_seconds_and_switches_to_address_batches() {
        let mut recalibration = Recalibration::new();
        assert_eq!(recalibration.decide(0), Decision::StartBatch);
        assert_eq!(
            recalibration.advance(Mode::Single, 10, 6, 15),
            Advance::NextBatch
        );
        assert_eq!(
            recalibration.advance(Mode::Single, 20, 6, 15),
            Advance::NextBatch
        );
        assert_eq!(
            recalibration.advance(Mode::Single, 30, 6, 15),
            Advance::PassComplete
        );
        assert!(recalibration.has_completed(Mode::Single));
        assert_eq!(recalibration.batching(), Batching::Addresses);
        assert_eq!(recalibration.decide(20_029), Decision::Wait);
        assert_eq!(recalibration.decide(20_030), Decision::StartBatch);
    }

    #[test]
    fn the_batching_after_a_mode_switch_depends_on_the_passes_done() {
        let mut recalibration = Recalibration::new();
        recalibration.restart(Mode::Parallel);
        assert_eq!(recalibration.batching(), Batching::InputPairs);
        recalibration.done_parallel = true;
        recalibration.restart(Mode::Single);
        assert_eq!(recalibration.batching(), Batching::InputPairs);
        recalibration.done_single = true;
        recalibration.restart(Mode::Single);
        assert_eq!(recalibration.batching(), Batching::Addresses);
        recalibration.restart(Mode::Parallel);
        assert_eq!(recalibration.batching(), Batching::Addresses);
    }
}
