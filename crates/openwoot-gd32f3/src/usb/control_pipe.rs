use core::{future::poll_fn, sync::atomic::Ordering, task::Poll};

use embassy_usb_driver::{ControlPipe as ControlPipeTrait, Direction, EndpointError, EndpointType};

use super::{
    endpoint::{Endpoint, In, Out, idle},
    endpoint_register::{Command, Reading, Status, Toggle},
    events::EVENTS,
    registers::{self, ADDR_USBEN},
};
use crate::register::write_bits;

const CONTROL: usize = 0;
const SETUP_LEN: usize = 8;
const GET_DESCRIPTOR: [u8; 2] = [0x80, 0x06];
const DEVICE_QUALIFIER: u8 = 0x06;
const OTHER_SPEED_CONFIGURATION: u8 = 0x07;

#[derive(Debug)]
pub struct ControlPipe {
    max_packet_size: u16,
    input: Endpoint<In>,
    output: Endpoint<Out>,
}

impl ControlPipe {
    pub(super) const fn new(
        max_packet_size: u16,
        input: Endpoint<In>,
        output: Endpoint<Out>,
    ) -> Self {
        Self {
            max_packet_size,
            input,
            output,
        }
    }
}

impl ControlPipeTrait for ControlPipe {
    fn max_packet_size(&self) -> usize {
        usize::from(self.max_packet_size)
    }

    async fn setup(&mut self) -> [u8; 8] {
        loop {
            poll_fn(|context| {
                EVENTS.register(CONTROL, Direction::Out, context);
                if EVENTS.setup.load(Ordering::Acquire) {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
            .await;
            EVENTS.setup.store(false, Ordering::Release);
            let mut request = [0; SETUP_LEN];
            match self.output.unload(&mut request) {
                Ok(SETUP_LEN) if asks_for_high_speed_only_descriptor(request) => {
                    registers::write_endpoint(CONTROL, reject(registers::endpoint(CONTROL)).bits());
                }
                Ok(SETUP_LEN) => return request,
                _ => {}
            }
        }
    }

    async fn data_out(
        &mut self,
        buf: &mut [u8],
        first: bool,
        last: bool,
    ) -> Result<usize, EndpointError> {
        if aborted() {
            return Err(EndpointError::Disabled);
        }
        if first || last {
            registers::write_endpoint(CONTROL, data_out_entry(first, last).bits());
        }
        poll_fn(|context| {
            EVENTS.register(CONTROL, Direction::Out, context);
            stage(
                aborted(),
                registers::endpoint(CONTROL).status(Direction::Out),
            )
        })
        .await?;
        let len = self.output.unload(buf)?;
        registers::write_endpoint(CONTROL, data_out_exit(last).bits());
        Ok(len)
    }

    async fn data_in(&mut self, data: &[u8], first: bool, last: bool) -> Result<(), EndpointError> {
        if data.len() > usize::from(self.max_packet_size) {
            return Err(EndpointError::BufferOverflow);
        }
        if aborted() {
            return Err(EndpointError::Disabled);
        }
        if first || last {
            registers::write_endpoint(CONTROL, data_in_entry(first, last).bits());
        }
        poll_fn(|context| {
            EVENTS.register(CONTROL, Direction::Out, context);
            EVENTS.register(CONTROL, Direction::In, context);
            stage(
                aborted(),
                registers::endpoint(CONTROL).status(Direction::In),
            )
        })
        .await?;
        self.input.load(data)?;
        registers::write_endpoint(CONTROL, data_in_exit(last).bits());
        Ok(())
    }

    async fn accept(&mut self) {
        if aborted() {
            return;
        }
        if self.input.load(&[]).is_ok() {
            registers::write_endpoint(CONTROL, accept(registers::endpoint(CONTROL)).bits());
        }
        poll_fn(|context| {
            EVENTS.register(CONTROL, Direction::Out, context);
            EVENTS.register(CONTROL, Direction::In, context);
            if aborted() || idle(registers::endpoint(CONTROL).status(Direction::In)) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }

    async fn reject(&mut self) {
        if aborted() {
            return;
        }
        registers::write_endpoint(CONTROL, reject(registers::endpoint(CONTROL)).bits());
    }

    async fn accept_set_address(&mut self, addr: u8) {
        self.accept().await;
        if !aborted() {
            write_bits(registers::usbd().addr(), ADDR_USBEN | u16::from(addr));
        }
    }
}

fn aborted() -> bool {
    EVENTS.setup.load(Ordering::Acquire) || EVENTS.reset.load(Ordering::Acquire)
}

fn stage(aborted: bool, status: Status) -> Poll<Result<(), EndpointError>> {
    if aborted || status == Status::Disabled {
        Poll::Ready(Err(EndpointError::Disabled))
    } else if status == Status::Nak {
        Poll::Ready(Ok(()))
    } else {
        Poll::Pending
    }
}

fn control() -> Command {
    Command::new(EndpointType::Control, CONTROL)
}

fn data_out_entry(first: bool, last: bool) -> Command {
    let mut receive = Toggle::NONE;
    let mut transmit = Toggle::NONE;
    if first {
        receive = Toggle::between(Status::Nak, Status::Valid);
        transmit = Toggle::between(Status::Nak, Status::Stall);
    }
    if last {
        transmit = transmit.then(Toggle::between(Status::Stall, Status::Nak));
    }
    control().toggle_receive(receive).toggle_transmit(transmit)
}

fn data_out_exit(last: bool) -> Command {
    let next = if last { Status::Stall } else { Status::Valid };
    control().toggle_receive(Toggle::between(Status::Nak, next))
}

fn data_in_entry(first: bool, last: bool) -> Command {
    let mut receive = Toggle::NONE;
    if first {
        receive = Toggle::between(Status::Nak, Status::Stall);
    }
    if last {
        receive = receive.then(Toggle::between(Status::Stall, Status::Valid));
    }
    control().toggle_receive(receive).with_status_out(last)
}

fn data_in_exit(last: bool) -> Command {
    control()
        .toggle_transmit(Toggle::between(Status::Nak, Status::Valid))
        .with_status_out(last)
}

fn accept(current: Reading) -> Command {
    control()
        .toggle_receive(Toggle::between(
            current.status(Direction::Out),
            Status::Stall,
        ))
        .toggle_transmit(Toggle::between(
            current.status(Direction::In),
            Status::Valid,
        ))
}

fn reject(current: Reading) -> Command {
    control()
        .toggle_receive(Toggle::between(
            current.status(Direction::Out),
            Status::Stall,
        ))
        .toggle_transmit(Toggle::between(
            current.status(Direction::In),
            Status::Stall,
        ))
}

fn asks_for_high_speed_only_descriptor(request: [u8; SETUP_LEN]) -> bool {
    match request {
        [request_type, code, _, descriptor, ..] => {
            [request_type, code] == GET_DESCRIPTOR
                && matches!(descriptor, DEVICE_QUALIFIER | OTHER_SPEED_CONFIGURATION)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::endpoint_register::tests::{after_setup, hardware_write, status_out};

    struct Model(u16);

    impl Model {
        fn apply(&mut self, command: Command) {
            self.0 = hardware_write(self.0, command.bits());
        }

        fn finish(&mut self, direction: Direction) {
            let reading = Reading(self.0);
            self.0 = hardware_write(self.0, reading.change_status(direction, Status::Nak));
        }

        fn statuses(&self) -> (Status, Status) {
            let reading = Reading(self.0);
            (
                reading.status(Direction::Out),
                reading.status(Direction::In),
            )
        }
    }

    #[test]
    fn a_single_packet_in_stage_sends_and_expects_the_status_stage() {
        let mut endpoint = Model(after_setup());
        endpoint.apply(data_in_entry(true, true));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Nak));
        endpoint.apply(data_in_exit(true));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Valid));
        assert!(status_out(endpoint.0));
    }

    #[test]
    fn a_multi_packet_in_stage_stalls_an_early_status_stage() {
        let mut endpoint = Model(after_setup());
        endpoint.apply(data_in_entry(true, false));
        endpoint.apply(data_in_exit(false));
        assert_eq!(endpoint.statuses(), (Status::Stall, Status::Valid));
        assert!(!status_out(endpoint.0));
        endpoint.finish(Direction::In);
        endpoint.apply(data_in_entry(false, true));
        endpoint.apply(data_in_exit(true));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Valid));
        assert!(status_out(endpoint.0));
    }

    #[test]
    fn an_out_stage_receives_then_waits_for_the_answer() {
        let mut endpoint = Model(after_setup());
        endpoint.apply(data_out_entry(true, false));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Stall));
        endpoint.finish(Direction::Out);
        endpoint.apply(data_out_exit(false));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Stall));
        endpoint.apply(data_out_entry(false, true));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Nak));
        endpoint.finish(Direction::Out);
        endpoint.apply(data_out_exit(true));
        assert_eq!(endpoint.statuses(), (Status::Stall, Status::Nak));
        endpoint.apply(accept(Reading(endpoint.0)));
        assert_eq!(endpoint.statuses(), (Status::Stall, Status::Valid));
    }

    #[test]
    fn a_single_packet_out_stage_keeps_the_in_direction_waiting() {
        let mut endpoint = Model(after_setup());
        endpoint.apply(data_out_entry(true, true));
        assert_eq!(endpoint.statuses(), (Status::Valid, Status::Nak));
    }

    #[test]
    fn accept_and_reject_reach_their_states_from_any_state() {
        for (out, input) in [(Status::Nak, Status::Nak), (Status::Valid, Status::Stall)] {
            let mut endpoint = Model(after_setup());
            endpoint.0 = hardware_write(
                endpoint.0,
                control()
                    .toggle_receive(Toggle::between(Status::Nak, out))
                    .toggle_transmit(Toggle::between(Status::Nak, input))
                    .bits(),
            );
            let mut rejected = Model(endpoint.0);
            endpoint.apply(accept(Reading(endpoint.0)));
            assert_eq!(endpoint.statuses(), (Status::Stall, Status::Valid));
            rejected.apply(reject(Reading(rejected.0)));
            assert_eq!(rejected.statuses(), (Status::Stall, Status::Stall));
        }
    }

    #[test]
    fn a_waiting_stage_ends_on_a_bus_reset_or_a_new_setup() {
        let after_bus_reset = Reading(0);
        for direction in [Direction::Out, Direction::In] {
            assert_eq!(
                stage(false, after_bus_reset.status(direction)),
                Poll::Ready(Err(EndpointError::Disabled))
            );
        }
        for status in [Status::Stall, Status::Nak, Status::Valid] {
            assert_eq!(
                stage(true, status),
                Poll::Ready(Err(EndpointError::Disabled))
            );
        }
    }

    #[test]
    fn a_waiting_stage_goes_on_only_when_the_endpoint_answers_nak() {
        assert_eq!(stage(false, Status::Nak), Poll::Ready(Ok(())));
        assert_eq!(stage(false, Status::Valid), Poll::Pending);
        assert_eq!(stage(false, Status::Stall), Poll::Pending);
    }

    #[test]
    fn high_speed_only_descriptors_are_refused_and_others_pass() {
        let device_qualifier = [0x80, 0x06, 0x00, 0x06, 0x00, 0x00, 0x0A, 0x00];
        let other_speed = [0x80, 0x06, 0x00, 0x07, 0x00, 0x00, 0x09, 0x00];
        let device = [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 0x12, 0x00];
        let class_request = [0xA1, 0x06, 0x00, 0x06, 0x00, 0x00, 0x0A, 0x00];
        assert!(asks_for_high_speed_only_descriptor(device_qualifier));
        assert!(asks_for_high_speed_only_descriptor(other_speed));
        assert!(!asks_for_high_speed_only_descriptor(device));
        assert!(!asks_for_high_speed_only_descriptor(class_request));
    }
}
