use core::{cell::RefCell, task::Waker};

use critical_section::{CriticalSection, Mutex};
use embassy_time_driver::Driver;
use embassy_time_queue_utils::Queue;

use super::{
    monotonic::{Counter, Events, Monotonic},
    registers,
};
use crate::{
    pac::interrupt,
    register::{modify_bits, read_bits, write_bits},
};

const INTF_UPIF: u32 = 1 << 0;
const INTF_CH0IF: u32 = 1 << 1;
const INTF_CH1IF: u32 = 1 << 2;
const DMAINTEN_CH1IE: u32 = 1 << 2;

struct Timer4;

impl Counter for Timer4 {
    fn count(&self) -> u32 {
        read_bits(registers().cnt())
    }

    fn set_alarm_compare(&self, value: u32) {
        let registers = registers();
        write_bits(registers.ch1cv(), value);
        write_bits(registers.intf(), !INTF_CH1IF);
    }

    fn set_alarm_interrupt(&self, enabled: bool) {
        modify_bits(registers().dmainten(), |interrupts| {
            if enabled {
                interrupts | DMAINTEN_CH1IE
            } else {
                interrupts & !DMAINTEN_CH1IE
            }
        });
    }

    fn take_events(&self) -> Events {
        let registers = registers();
        let flags = read_bits(registers.intf());
        let enabled = read_bits(registers.dmainten());
        write_bits(registers.intf(), !flags);
        Events {
            overflow: flags & INTF_UPIF != 0,
            half: flags & INTF_CH0IF != 0,
            alarm: flags & INTF_CH1IF != 0 && enabled & DMAINTEN_CH1IE != 0,
        }
    }
}

struct TimeDriver {
    monotonic: Monotonic<Timer4>,
    queue: Mutex<RefCell<Queue>>,
}

impl TimeDriver {
    fn on_interrupt(&self) {
        critical_section::with(|cs| {
            if self.monotonic.on_interrupt(cs) {
                self.arm_next(cs, &mut self.queue.borrow(cs).borrow_mut());
            }
        });
    }

    fn arm_next(&self, cs: CriticalSection<'_>, queue: &mut Queue) {
        let mut next = queue.next_expiration(self.monotonic.now());
        while !self.monotonic.set_alarm(cs, next) {
            next = queue.next_expiration(self.monotonic.now());
        }
    }
}

impl Driver for TimeDriver {
    fn now(&self) -> u64 {
        self.monotonic.now()
    }

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        critical_section::with(|cs| {
            let mut queue = self.queue.borrow(cs).borrow_mut();
            if queue.schedule_wake(at, waker) {
                self.arm_next(cs, &mut queue);
            }
        });
    }
}

embassy_time_driver::time_driver_impl!(static DRIVER: TimeDriver = TimeDriver {
    monotonic: Monotonic::new(Timer4),
    queue: Mutex::new(RefCell::new(Queue::new())),
});

#[interrupt]
fn TIMER4() {
    DRIVER.on_interrupt();
}
