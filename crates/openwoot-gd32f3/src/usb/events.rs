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
        EVENTS.suspend.store(true, Ordering::Release);
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
        EVENTS.resume.store(true, Ordering::Release);
        modify_bits(usbd.ctl(), |control| control & !(CTL_LOWM | CTL_SETSPS));
        write_bits(usbd.intf(), !INTF_WKUPIF);
        EVENTS.bus.wake();
    }

    if flags & INTF_RSTIF != 0 {
        EVENTS.reset.store(true, Ordering::Release);
        write_bits(usbd.intf(), !INTF_RSTIF);
        EVENTS.bus.wake();
    }

    if flags & INTF_STIF != 0 {
        let index = usize::from(flags & INTF_EPNUM);
        let endpoint = registers::endpoint(index);
        if endpoint.received() {
            if index == 0 && endpoint.setup() {
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
