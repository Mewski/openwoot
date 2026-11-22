use core::{
    convert::Infallible,
    future::poll_fn,
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};

use embassy_time::Timer;
use embassy_usb_driver::{
    Bus as BusTrait, Direction, EndpointAddress, EndpointType, Event, Speed, Unsupported,
};
use embedded_hal::digital::OutputPin;

use super::{
    endpoint_register::{Command, Status, Toggle},
    events,
    packet_memory::ENDPOINT_COUNT,
    registers::{self, ADDR_USBEN, CTL_LOWM, CTL_RSREQ, CTL_SETSPS},
};
use crate::register::{clear_bits, modify_bits, write_bits};

const RESUME_SIGNALLING_MILLISECONDS: u64 = 4;

static ENUMERATED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct Bus<PullUp> {
    pull_up: PullUp,
    endpoint_types: [EndpointType; ENDPOINT_COUNT],
    powered: bool,
}

#[must_use]
pub fn negotiated_speed() -> Option<Speed> {
    ENUMERATED.load(Ordering::Acquire).then_some(Speed::Full)
}

impl<PullUp: OutputPin<Error = Infallible>> Bus<PullUp> {
    pub(crate) const fn new(
        pull_up: PullUp,
        endpoint_types: [EndpointType; ENDPOINT_COUNT],
    ) -> Self {
        Self {
            pull_up,
            endpoint_types,
            powered: false,
        }
    }

    fn reset_endpoints(&self) {
        write_bits(registers::usbd().addr(), ADDR_USBEN);
        registers::write_endpoint(
            0,
            Command::new(EndpointType::Control, 0)
                .toggle_receive(Toggle::between(Status::Disabled, Status::Nak))
                .toggle_transmit(Toggle::between(Status::Disabled, Status::Nak))
                .bits(),
        );
        for (index, endpoint_type) in self.endpoint_types.iter().enumerate().skip(1) {
            registers::write_endpoint(index, Command::new(*endpoint_type, index).bits());
        }
        events::wake_endpoints();
    }
}

impl<PullUp: OutputPin<Error = Infallible>> BusTrait for Bus<PullUp> {
    async fn enable(&mut self) {
        let Ok(()) = self.pull_up.set_high();
    }

    async fn disable(&mut self) {
        let Ok(()) = self.pull_up.set_low();
        self.powered = false;
    }

    async fn poll(&mut self) -> Event {
        poll_fn(|context| {
            events::BUS.register(context.waker());
            if !self.powered {
                self.powered = true;
                return Poll::Ready(Event::PowerDetected);
            }
            if events::RESUME.swap(false, Ordering::AcqRel) {
                return Poll::Ready(Event::Resume);
            }
            if events::RESET.swap(false, Ordering::AcqRel) {
                self.reset_endpoints();
                ENUMERATED.store(true, Ordering::Release);
                return Poll::Ready(Event::Reset);
            }
            if events::SUSPEND.swap(false, Ordering::AcqRel) {
                return Poll::Ready(Event::Suspend);
            }
            Poll::Pending
        })
        .await
    }

    fn endpoint_set_enabled(&mut self, ep_addr: EndpointAddress, enabled: bool) {
        let direction = ep_addr.direction();
        let wanted = match (enabled, direction) {
            (false, _) => Status::Disabled,
            (true, Direction::In) => Status::Nak,
            (true, Direction::Out) => Status::Valid,
        };
        settle(ep_addr, wanted, enabled);
    }

    fn endpoint_set_stalled(&mut self, ep_addr: EndpointAddress, stalled: bool) {
        let index = ep_addr.index();
        let direction = ep_addr.direction();
        if registers::endpoint(index).status(direction) == Status::Disabled {
            return;
        }
        let wanted = match (stalled, direction) {
            (true, _) => Status::Stall,
            (false, Direction::In) => Status::Nak,
            (false, Direction::Out) => Status::Valid,
        };
        settle(ep_addr, wanted, !stalled);
    }

    fn endpoint_is_stalled(&mut self, ep_addr: EndpointAddress) -> bool {
        registers::endpoint(ep_addr.index()).status(ep_addr.direction()) == Status::Stall
    }

    async fn remote_wakeup(&mut self) -> Result<(), Unsupported> {
        let control = registers::usbd().ctl();
        critical_section::with(|_| {
            modify_bits(control, |bits| {
                (bits & !(CTL_LOWM | CTL_SETSPS)) | CTL_RSREQ
            });
        });
        Timer::after_millis(RESUME_SIGNALLING_MILLISECONDS).await;
        critical_section::with(|_| clear_bits(control, CTL_RSREQ));
        Ok(())
    }
}

fn settle(ep_addr: EndpointAddress, wanted: Status, restart: bool) {
    let index = ep_addr.index();
    let direction = ep_addr.direction();
    let mut current = registers::endpoint(index);
    if restart && (current.status(direction) != wanted || current.data_toggle(direction)) {
        registers::write_endpoint(index, current.restart(direction, wanted));
        current = registers::endpoint(index);
    }
    while current.status(direction) != wanted {
        registers::write_endpoint(index, current.change_status(direction, wanted));
        current = registers::endpoint(index);
    }
    events::wake(events::waker(index, direction));
}
