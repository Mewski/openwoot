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

pub(crate) static BUS: AtomicWaker = AtomicWaker::new();
pub(crate) static SETUP: AtomicBool = AtomicBool::new(false);
pub(crate) static RESET: AtomicBool = AtomicBool::new(false);
pub(crate) static SUSPEND: AtomicBool = AtomicBool::new(false);
pub(crate) static RESUME: AtomicBool = AtomicBool::new(false);
static IN: [AtomicWaker; ENDPOINT_COUNT] = [const { AtomicWaker::new() }; ENDPOINT_COUNT];
static OUT: [AtomicWaker; ENDPOINT_COUNT] = [const { AtomicWaker::new() }; ENDPOINT_COUNT];

#[derive(Debug)]
pub struct InterruptHandler;

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

pub(crate) fn waker(index: usize, direction: Direction) -> Option<&'static AtomicWaker> {
    match direction {
        Direction::In => IN.get(index),
        Direction::Out => OUT.get(index),
    }
}

pub(crate) fn wake_endpoints() {
    IN.iter().chain(&OUT).for_each(AtomicWaker::wake);
}

pub(crate) fn wake(waker: Option<&AtomicWaker>) {
    if let Some(waker) = waker {
        waker.wake();
    }
}

pub(crate) fn register(waker: Option<&AtomicWaker>, context: &Context<'_>) {
    if let Some(waker) = waker {
        waker.register(context.waker());
    }
}

fn service() {
    let usbd = registers::usbd();
    let flags = read_bits(usbd.intf());

    if flags & INTF_SPSIF != 0 {
        SUSPEND.store(true, Ordering::Release);
        modify_bits(usbd.ctl(), |control| {
            if control & CTL_RSREQ == 0 {
                control | CTL_SETSPS | CTL_LOWM
            } else {
                control
            }
        });
        write_bits(usbd.intf(), !INTF_SPSIF);
        BUS.wake();
    }

    if flags & INTF_WKUPIF != 0 {
        RESUME.store(true, Ordering::Release);
        modify_bits(usbd.ctl(), |control| control & !(CTL_LOWM | CTL_SETSPS));
        write_bits(usbd.intf(), !INTF_WKUPIF);
        BUS.wake();
    }

    if flags & INTF_RSTIF != 0 {
        RESET.store(true, Ordering::Release);
        write_bits(usbd.intf(), !INTF_RSTIF);
        BUS.wake();
    }

    if flags & INTF_STIF != 0 {
        let index = usize::from(flags & INTF_EPNUM);
        let endpoint = registers::endpoint(index);
        if endpoint.received() {
            if index == 0 && endpoint.setup() {
                SETUP.store(true, Ordering::Release);
            }
            wake(waker(index, Direction::Out));
        }
        if endpoint.transmitted() {
            wake(waker(index, Direction::In));
        }
        registers::write_endpoint(index, endpoint.acknowledge());
    }
}
