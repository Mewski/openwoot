use core::{convert::Infallible, future::poll_fn, sync::atomic::Ordering, task::Poll};

use embassy_time::Timer;
use embassy_usb_driver::{
    Bus as BusTrait, Direction, EndpointAddress, EndpointType, Event, Unsupported,
};
use embedded_hal::digital::OutputPin;

use super::{
    endpoint_register::{Command, Reading, Status, Toggle},
    events::EVENTS,
    packet_memory::ENDPOINT_COUNT,
    registers::{self, ADDR_USBEN, CTL_LOWM, CTL_RSREQ, CTL_SETSPS},
    speed,
};
use crate::register::{clear_bits, modify_bits, write_bits};

const CONTROL: usize = 0;
const RESUME_SIGNALLING_MILLISECONDS: u64 = 4;

#[derive(Debug)]
pub struct Bus<PullUp> {
    pull_up: PullUp,
    endpoint_types: [EndpointType; ENDPOINT_COUNT],
    powered: bool,
}

impl<PullUp: OutputPin<Error = Infallible>> Bus<PullUp> {
    pub(super) const fn new(
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
        for (index, endpoint_type) in self.endpoint_types.iter().enumerate() {
            let wanted = if index == CONTROL {
                Status::Nak
            } else {
                Status::Disabled
            };
            let command = reset_command(*endpoint_type, index, registers::endpoint(index), wanted);
            registers::write_endpoint(index, command.bits());
        }
        EVENTS.wake_endpoints();
    }
}

impl<PullUp: OutputPin<Error = Infallible>> BusTrait for Bus<PullUp> {
    async fn enable(&mut self) {
        EVENTS.reset.store(false, Ordering::Release);
        let Ok(()) = self.pull_up.set_high();
    }

    async fn disable(&mut self) {
        let Ok(()) = self.pull_up.set_low();
        self.powered = false;
    }

    async fn poll(&mut self) -> Event {
        poll_fn(|context| {
            EVENTS.bus.register(context.waker());
            if !self.powered {
                self.powered = true;
                return Poll::Ready(Event::PowerDetected);
            }
            if EVENTS.resume.swap(false, Ordering::AcqRel) {
                return Poll::Ready(Event::Resume);
            }
            if EVENTS.reset.swap(false, Ordering::AcqRel) {
                self.reset_endpoints();
                speed::record();
                return Poll::Ready(Event::Reset);
            }
            if EVENTS.suspend.swap(false, Ordering::AcqRel) {
                return Poll::Ready(Event::Suspend);
            }
            Poll::Pending
        })
        .await
    }

    fn endpoint_set_enabled(&mut self, ep_addr: EndpointAddress, enabled: bool) {
        if !exists(ep_addr) {
            return;
        }
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
        if !exists(ep_addr) || registers::endpoint(index).status(direction) == Status::Disabled {
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
        exists(ep_addr)
            && registers::endpoint(ep_addr.index()).status(ep_addr.direction()) == Status::Stall
    }

    async fn remote_wakeup(&mut self) -> Result<(), Unsupported> {
        let control = registers::usbd().ctl();
        critical_section::with(|_| {
            modify_bits(control, |bits| {
                (bits & !(CTL_LOWM | CTL_SETSPS)) | CTL_RSREQ
            });
        });
        let request = ResumeRequest;
        Timer::after_millis(RESUME_SIGNALLING_MILLISECONDS).await;
        drop(request);
        Ok(())
    }
}

struct ResumeRequest;

impl Drop for ResumeRequest {
    fn drop(&mut self) {
        critical_section::with(|_| clear_bits(registers::usbd().ctl(), CTL_RSREQ));
    }
}

fn exists(ep_addr: EndpointAddress) -> bool {
    ep_addr.index() < ENDPOINT_COUNT
}

fn reset_command(
    endpoint_type: EndpointType,
    index: usize,
    current: Reading,
    wanted: Status,
) -> Command {
    Command::new(endpoint_type, index)
        .toggle_receive(Toggle::between(current.status(Direction::Out), wanted))
        .toggle_transmit(Toggle::between(current.status(Direction::In), wanted))
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
    EVENTS.wake(index, direction);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::endpoint_register::tests::hardware_write;

    const STATUSES: [Status; 4] = [Status::Disabled, Status::Stall, Status::Nak, Status::Valid];

    fn register(receive: Status, transmit: Status) -> u16 {
        let command = Command::new(EndpointType::Bulk, 0)
            .toggle_receive(Toggle::between(Status::Disabled, receive))
            .toggle_transmit(Toggle::between(Status::Disabled, transmit));
        hardware_write(0, command.bits())
    }

    #[test]
    fn a_bus_reset_settles_the_control_endpoint_from_whatever_a_late_write_left() {
        for receive in STATUSES {
            for transmit in STATUSES {
                let before = register(receive, transmit);
                let command =
                    reset_command(EndpointType::Control, CONTROL, Reading(before), Status::Nak);
                let after = Reading(hardware_write(before, command.bits()));
                assert_eq!(after.status(Direction::Out), Status::Nak);
                assert_eq!(after.status(Direction::In), Status::Nak);
            }
        }
    }

    #[test]
    fn a_bus_reset_leaves_the_other_endpoints_disabled_with_their_type_and_address() {
        for receive in STATUSES {
            for transmit in STATUSES {
                let before = register(receive, transmit);
                let command = reset_command(
                    EndpointType::Interrupt,
                    3,
                    Reading(before),
                    Status::Disabled,
                );
                let after = Reading(hardware_write(before, command.bits()));
                assert_eq!(after.status(Direction::Out), Status::Disabled);
                assert_eq!(after.status(Direction::In), Status::Disabled);
                assert_eq!(
                    after.0,
                    hardware_write(0, Command::new(EndpointType::Interrupt, 3).bits())
                );
            }
        }
    }

    #[test]
    fn endpoint_numbers_beyond_the_registers_do_not_exist() {
        for index in 0..ENDPOINT_COUNT {
            assert!(exists(EndpointAddress::from_parts(index, Direction::In)));
            assert!(exists(EndpointAddress::from_parts(index, Direction::Out)));
        }
        for index in ENDPOINT_COUNT..16 {
            assert!(!exists(EndpointAddress::from_parts(index, Direction::In)));
            assert!(!exists(EndpointAddress::from_parts(index, Direction::Out)));
        }
    }
}
