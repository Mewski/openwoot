use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::Context,
};

use embassy_sync::waitqueue::AtomicWaker;
use embassy_usb_driver::Direction;

use super::{
    packet_memory::ENDPOINT_COUNT,
    registers::{
        self, CTL_LOWM, CTL_RSREQ, CTL_SETSPS, INTF_EPNUM, INTF_RSTIF, INTF_SPSIF, INTF_STIF,
        INTF_WKUPIF,
    },
};
use crate::{
    interrupt::{Handler, typelevel},
    register::{modify_bits, read_bits, write_bits},
};

pub(super) static EVENTS: Events = Events::new();

const CONTROL: usize = 0;

#[derive(Debug)]
pub struct InterruptHandler;

pub(super) struct Events {
    pub(super) bus: AtomicWaker,
    pub(super) setup: AtomicBool,
    pub(super) reset: AtomicBool,
    pub(super) suspend: AtomicBool,
    pub(super) resume: AtomicBool,
    in_endpoints: [AtomicWaker; ENDPOINT_COUNT],
    out_endpoints: [AtomicWaker; ENDPOINT_COUNT],
}

impl Events {
    const fn new() -> Self {
        Self {
            bus: AtomicWaker::new(),
            setup: AtomicBool::new(false),
            reset: AtomicBool::new(false),
            suspend: AtomicBool::new(false),
            resume: AtomicBool::new(false),
            in_endpoints: [const { AtomicWaker::new() }; ENDPOINT_COUNT],
            out_endpoints: [const { AtomicWaker::new() }; ENDPOINT_COUNT],
        }
    }

    fn note_suspend(&self) {
        self.suspend.store(true, Ordering::Release);
    }

    fn note_resume(&self) {
        self.suspend.store(false, Ordering::Release);
        self.resume.store(true, Ordering::Release);
    }

    fn note_reset(&self) {
        self.suspend.store(false, Ordering::Release);
        self.setup.store(false, Ordering::Release);
        self.reset.store(true, Ordering::Release);
    }

    pub(super) fn wake_endpoints(&self) {
        self.in_endpoints
            .iter()
            .chain(&self.out_endpoints)
            .for_each(AtomicWaker::wake);
    }

    pub(super) fn wake(&self, index: usize, direction: Direction) {
        if let Some(waker) = self.waker(index, direction) {
            waker.wake();
        }
    }

    pub(super) fn register(&self, index: usize, direction: Direction, context: &Context<'_>) {
        if let Some(waker) = self.waker(index, direction) {
            waker.register(context.waker());
        }
    }

    fn waker(&self, index: usize, direction: Direction) -> Option<&AtomicWaker> {
        match direction {
            Direction::In => self.in_endpoints.get(index),
            Direction::Out => self.out_endpoints.get(index),
        }
    }
}

impl Handler<typelevel::USBD_HP_CAN0_TX> for InterruptHandler {
    // SAFETY: servicing the controller has no requirement beyond running in a USB interrupt.
    unsafe fn on_interrupt() {
        service();
    }
}

impl Handler<typelevel::USBD_LP_CAN0_RX0> for InterruptHandler {
    // SAFETY: servicing the controller has no requirement beyond running in a USB interrupt.
    unsafe fn on_interrupt() {
        service();
    }
}

fn service() {
    let usbd = registers::usbd();
    let flags = read_bits(usbd.intf());

    if flags & INTF_SPSIF != 0 {
        EVENTS.note_suspend();
        modify_bits(usbd.ctl(), |control| {
            if control & CTL_RSREQ == 0 {
                control | CTL_SETSPS | CTL_LOWM
            } else {
                control
            }
        });
        write_bits(usbd.intf(), !INTF_SPSIF);
        EVENTS.bus.wake();
    }

    if flags & INTF_WKUPIF != 0 {
        EVENTS.note_resume();
        modify_bits(usbd.ctl(), |control| control & !(CTL_LOWM | CTL_SETSPS));
        write_bits(usbd.intf(), !INTF_WKUPIF);
        EVENTS.bus.wake();
    }

    if flags & INTF_RSTIF != 0 {
        EVENTS.note_reset();
        modify_bits(usbd.ctl(), |control| control & !(CTL_LOWM | CTL_SETSPS));
        write_bits(usbd.intf(), !INTF_RSTIF);
        EVENTS.bus.wake();
        EVENTS.wake(CONTROL, Direction::Out);
        EVENTS.wake(CONTROL, Direction::In);
    }

    if flags & INTF_STIF != 0 {
        let index = usize::from(flags & INTF_EPNUM);
        let endpoint = registers::endpoint(index);
        if endpoint.received() {
            if index == CONTROL && endpoint.setup() {
                EVENTS.setup.store(true, Ordering::Release);
            }
            EVENTS.wake(index, Direction::Out);
        }
        if endpoint.transmitted() {
            EVENTS.wake(index, Direction::In);
        }
        registers::write_endpoint(index, endpoint.acknowledge());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(events: &Events) -> [bool; 4] {
        [
            events.setup.load(Ordering::Acquire),
            events.reset.load(Ordering::Acquire),
            events.suspend.load(Ordering::Acquire),
            events.resume.load(Ordering::Acquire),
        ]
    }

    #[test]
    fn a_wake_up_supersedes_a_suspend_that_was_not_reported_yet() {
        let events = Events::new();
        events.note_suspend();
        events.note_resume();
        assert_eq!(pending(&events), [false, false, false, true]);
    }

    #[test]
    fn a_bus_reset_supersedes_a_pending_suspend_and_a_pending_setup() {
        let events = Events::new();
        events.setup.store(true, Ordering::Release);
        events.note_suspend();
        events.note_reset();
        assert_eq!(pending(&events), [false, true, false, false]);
    }

    #[test]
    fn a_suspend_behind_a_bus_reset_stays_pending() {
        let events = Events::new();
        events.note_reset();
        events.note_suspend();
        assert_eq!(pending(&events), [false, true, true, false]);
    }
}
