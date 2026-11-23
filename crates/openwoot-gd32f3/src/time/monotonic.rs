use core::{
    cell::Cell,
    sync::atomic::{AtomicU32, Ordering, compiler_fence},
};

use critical_section::{CriticalSection, Mutex};

use super::COUNTER_BITS;

pub(super) const HALF: u32 = 1 << (COUNTER_BITS - 1);
pub(super) const COUNTER_MAX: u32 = u32::MAX >> (32 - COUNTER_BITS);
const ARM_WINDOW: u64 = HALF as u64 + HALF as u64 / 2;

pub(super) trait Counter {
    fn count(&self) -> u32;
    fn set_alarm_compare(&self, value: u32);
    fn set_alarm_interrupt(&self, enabled: bool);
    fn take_events(&self) -> Events;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Events {
    pub(super) overflow: bool,
    pub(super) half: bool,
    pub(super) alarm: bool,
}

pub(super) struct Monotonic<Hardware> {
    counter: Hardware,
    period: AtomicU32,
    alarm: Mutex<Cell<u64>>,
}

impl<Hardware: Counter> Monotonic<Hardware> {
    pub(super) const fn new(counter: Hardware) -> Self {
        Self {
            counter,
            period: AtomicU32::new(0),
            alarm: Mutex::new(Cell::new(u64::MAX)),
        }
    }

    pub(super) fn now(&self) -> u64 {
        let period = self.period.load(Ordering::Relaxed);
        compiler_fence(Ordering::Acquire);
        timestamp(period, self.counter.count() & COUNTER_MAX)
    }

    pub(super) fn on_interrupt(&self, cs: CriticalSection<'_>) -> bool {
        let events = self.counter.take_events();
        if events.overflow {
            self.next_period(cs);
        }
        if events.half {
            self.next_period(cs);
        }
        if events.alarm {
            self.cancel_alarm(self.alarm.borrow(cs));
        }
        events.alarm
    }

    pub(super) fn set_alarm(&self, cs: CriticalSection<'_>, at: u64) -> bool {
        let alarm = self.alarm.borrow(cs);
        alarm.set(at);
        let now = self.now();
        if at <= now {
            self.cancel_alarm(alarm);
            return false;
        }
        self.counter.set_alarm_compare(compare_value(at));
        self.counter.set_alarm_interrupt(at - now < ARM_WINDOW);
        if at <= self.now() {
            self.cancel_alarm(alarm);
            return false;
        }
        true
    }

    fn cancel_alarm(&self, alarm: &Cell<u64>) {
        self.counter.set_alarm_interrupt(false);
        alarm.set(u64::MAX);
    }

    fn next_period(&self, cs: CriticalSection<'_>) {
        let period = self.period.load(Ordering::Relaxed).wrapping_add(1);
        self.period.store(period, Ordering::Relaxed);
        let start = u64::from(period) << (COUNTER_BITS - 1);
        if self.alarm.borrow(cs).get() < start + ARM_WINDOW {
            self.counter.set_alarm_interrupt(true);
        }
    }
}

const fn timestamp(period: u32, count: u32) -> u64 {
    ((period as u64) << (COUNTER_BITS - 1)) + (count ^ ((period & 1) << (COUNTER_BITS - 1))) as u64
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the compare register holds the low counter bits of the alarm time"
)]
const fn compare_value(at: u64) -> u32 {
    at as u32 & COUNTER_MAX
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::*;

    const PERIOD: u64 = 1 << COUNTER_BITS;

    #[derive(Default)]
    struct CounterModel {
        count: Cell<u32>,
        compare: Cell<u32>,
        alarm_enabled: Cell<bool>,
        pending: Cell<Events>,
    }

    impl CounterModel {
        fn ticks_to_next_event(&self) -> u64 {
            let count = u64::from(self.count.get());
            let until = |target: u32| {
                let distance = (u64::from(target) + PERIOD - count) % PERIOD;
                if distance == 0 { PERIOD } else { distance }
            };
            until(0).min(until(HALF)).min(until(self.compare.get()))
        }

        fn step(&self, ticks: u64) {
            let count = (u64::from(self.count.get()) + ticks) % PERIOD;
            let count = u32::try_from(count).unwrap();
            self.count.set(count);
            let mut pending = self.pending.get();
            pending.overflow |= count == 0;
            pending.half |= count == HALF;
            pending.alarm |= count == self.compare.get();
            self.pending.set(pending);
        }

        fn interrupt_pending(&self) -> bool {
            let pending = self.pending.get();
            pending.overflow || pending.half || (pending.alarm && self.alarm_enabled.get())
        }
    }

    impl Counter for Rc<CounterModel> {
        fn count(&self) -> u32 {
            self.count.get()
        }

        fn set_alarm_compare(&self, value: u32) {
            self.compare.set(value);
            let mut pending = self.pending.get();
            pending.alarm = false;
            self.pending.set(pending);
        }

        fn set_alarm_interrupt(&self, enabled: bool) {
            self.alarm_enabled.set(enabled);
        }

        fn take_events(&self) -> Events {
            let pending = self.pending.replace(Events::default());
            Events {
                alarm: pending.alarm && self.alarm_enabled.get(),
                ..pending
            }
        }
    }

    struct Machine {
        counter: Rc<CounterModel>,
        monotonic: Monotonic<Rc<CounterModel>>,
        elapsed: u64,
        alarms: Vec<u64>,
    }

    impl Machine {
        fn new() -> Self {
            let counter = Rc::new(CounterModel::default());
            Self {
                monotonic: Monotonic::new(Rc::clone(&counter)),
                counter,
                elapsed: 0,
                alarms: Vec::new(),
            }
        }

        fn advance(&mut self, ticks: u64) {
            let mut remaining = ticks;
            while remaining > 0 {
                let step = remaining.min(self.counter.ticks_to_next_event());
                self.counter.step(step);
                self.elapsed += step;
                remaining -= step;
                assert_eq!(self.monotonic.now(), self.elapsed);
                self.service();
            }
        }

        fn advance_without_service(&mut self, ticks: u64) {
            let mut remaining = ticks;
            while remaining > 0 {
                let step = remaining.min(self.counter.ticks_to_next_event());
                self.counter.step(step);
                self.elapsed += step;
                remaining -= step;
            }
        }

        fn service(&mut self) {
            if self.counter.interrupt_pending()
                && critical_section::with(|cs| self.monotonic.on_interrupt(cs))
            {
                self.alarms.push(self.monotonic.now());
            }
        }

        fn set_alarm(&self, at: u64) -> bool {
            critical_section::with(|cs| self.monotonic.set_alarm(cs, at))
        }
    }

    #[test]
    fn now_counts_ticks_across_many_counter_wraps() {
        let mut machine = Machine::new();
        for chunk in [
            1,
            u64::from(HALF) / 2 + 1,
            u64::from(HALF) - 1,
            7,
            u64::from(HALF) / 3,
        ] {
            for _ in 0..12 {
                machine.advance(chunk);
                assert_eq!(machine.monotonic.now(), machine.elapsed);
            }
        }
        assert!(machine.elapsed > 4 * PERIOD);
    }

    #[test]
    fn now_stays_right_while_a_counter_interrupt_is_still_pending() {
        let mut machine = Machine::new();
        machine.advance(PERIOD - 10);
        for _ in 0..8 {
            machine.advance_without_service(u64::from(HALF) / 2 - 1);
            assert_eq!(machine.monotonic.now(), machine.elapsed);
            machine.service();
            assert_eq!(machine.monotonic.now(), machine.elapsed);
        }
    }

    #[test]
    fn now_holds_until_an_unserviced_counter_event_is_half_a_counter_period_old() {
        let mut machine = Machine::new();
        machine.advance(PERIOD - 1);
        machine.advance_without_service(u64::from(HALF));
        assert_eq!(machine.monotonic.now(), machine.elapsed);
        machine.advance_without_service(1);
        assert_eq!(machine.monotonic.now(), machine.elapsed - PERIOD);
        machine.service();
        assert_eq!(machine.monotonic.now(), machine.elapsed);
    }

    #[test]
    fn an_alarm_that_is_due_is_refused() {
        let mut machine = Machine::new();
        machine.advance(5000);
        assert!(!machine.set_alarm(5000));
        assert!(!machine.set_alarm(4999));
        assert!(!machine.set_alarm(0));
        assert!(!machine.counter.alarm_enabled.get());
    }

    #[test]
    fn a_near_alarm_fires_exactly_at_its_time() {
        let mut machine = Machine::new();
        machine.advance(123);
        assert!(machine.set_alarm(1123));
        assert!(machine.counter.alarm_enabled.get());
        machine.advance(999);
        assert_eq!(machine.alarms, Vec::<u64>::new());
        machine.advance(1);
        assert_eq!(machine.alarms, [1123]);
        assert!(!machine.counter.alarm_enabled.get());
        machine.advance(2 * PERIOD);
        assert_eq!(machine.alarms, [1123]);
    }

    #[test]
    fn a_distant_alarm_waits_for_its_period_and_fires_once() {
        let mut machine = Machine::new();
        machine.advance(77);
        let at = 3 * PERIOD + u64::from(HALF) + 5;
        assert!(machine.set_alarm(at));
        assert!(!machine.counter.alarm_enabled.get());
        machine.advance(at - 77 - 1);
        assert_eq!(machine.alarms, Vec::<u64>::new());
        machine.advance(1);
        assert_eq!(machine.alarms, [at]);
        machine.advance(3 * PERIOD);
        assert_eq!(machine.alarms, [at]);
    }

    #[test]
    fn an_alarm_across_the_counter_wrap_fires_at_its_time() {
        let mut machine = Machine::new();
        machine.advance(PERIOD - 3);
        let at = PERIOD + 2;
        assert!(machine.set_alarm(at));
        machine.advance(10);
        assert_eq!(machine.alarms, [at]);
    }

    #[test]
    fn a_new_alarm_replaces_the_previous_one() {
        let mut machine = Machine::new();
        assert!(machine.set_alarm(500));
        assert!(machine.set_alarm(300));
        machine.advance(1000);
        assert_eq!(machine.alarms, [300]);
    }
}
