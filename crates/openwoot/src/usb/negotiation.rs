use embassy_usb::driver::{
    self, ControlPipe, Driver, EndpointAddress, EndpointAllocError, EndpointError, EndpointType,
};

use super::{Face, device::speed_dependent_interval};

const CONTROL_PACKET_CAPACITY: usize = 64;
const GET_DESCRIPTOR_REQUEST: [u8; 2] = [0x80, 0x06];
const CONFIGURATION_DESCRIPTOR_TYPE: u8 = 0x02;
const ENDPOINT_DESCRIPTOR_TYPE: u8 = 0x05;
const ENDPOINT_ADDRESS_OFFSET: u8 = 2;
const ENDPOINT_INTERVAL_OFFSET: u8 = 6;

pub struct Negotiated<D> {
    driver: D,
    face: Face,
    speed: fn() -> Option<driver::Speed>,
}

pub struct NegotiatedPipe<P> {
    pipe: P,
    face: Face,
    speed: fn() -> Option<driver::Speed>,
    rewrite: Option<IntervalRewrite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntervalRewrite {
    face: Face,
    position: u8,
    descriptor_len: u8,
    descriptor_type: u8,
    endpoint: u8,
}

impl<D> Negotiated<D> {
    pub const fn new(driver: D, face: Face, speed: fn() -> Option<driver::Speed>) -> Self {
        Self {
            driver,
            face,
            speed,
        }
    }
}

impl<'d, D: Driver<'d>> Driver<'d> for Negotiated<D> {
    type Bus = D::Bus;
    type ControlPipe = NegotiatedPipe<D::ControlPipe>;
    type EndpointIn = D::EndpointIn;
    type EndpointOut = D::EndpointOut;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        self.driver
            .alloc_endpoint_out(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn alloc_endpoint_in(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointIn, EndpointAllocError> {
        self.driver
            .alloc_endpoint_in(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn start(self, control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        let (bus, pipe) = self.driver.start(control_max_packet_size);
        (
            bus,
            NegotiatedPipe {
                pipe,
                face: self.face,
                speed: self.speed,
                rewrite: None,
            },
        )
    }
}

impl<P: ControlPipe> ControlPipe for NegotiatedPipe<P> {
    fn max_packet_size(&self) -> usize {
        self.pipe.max_packet_size()
    }

    async fn setup(&mut self) -> [u8; 8] {
        let setup = self.pipe.setup().await;
        self.rewrite = match ((self.speed)(), setup) {
            (Some(speed), [request_type, request, _, CONFIGURATION_DESCRIPTOR_TYPE, ..])
                if [request_type, request] == GET_DESCRIPTOR_REQUEST =>
            {
                Some(IntervalRewrite::new(Face {
                    speed: speed.into(),
                    ..self.face
                }))
            }
            _ => None,
        };
        setup
    }

    async fn data_out(
        &mut self,
        buf: &mut [u8],
        first: bool,
        last: bool,
    ) -> Result<usize, EndpointError> {
        self.pipe.data_out(buf, first, last).await
    }

    async fn data_in(&mut self, data: &[u8], first: bool, last: bool) -> Result<(), EndpointError> {
        let mut packet = [0; CONTROL_PACKET_CAPACITY];
        match (&mut self.rewrite, packet.get_mut(..data.len())) {
            (Some(rewrite), Some(packet)) => {
                packet.copy_from_slice(data);
                for byte in packet.iter_mut() {
                    rewrite.advance(byte);
                }
                self.pipe.data_in(packet, first, last).await
            }
            _ => self.pipe.data_in(data, first, last).await,
        }
    }

    async fn accept(&mut self) {
        self.pipe.accept().await;
    }

    async fn reject(&mut self) {
        self.pipe.reject().await;
    }

    async fn accept_set_address(&mut self, addr: u8) {
        self.pipe.accept_set_address(addr).await;
    }
}

impl IntervalRewrite {
    const fn new(face: Face) -> Self {
        Self {
            face,
            position: 0,
            descriptor_len: 0,
            descriptor_type: 0,
            endpoint: 0,
        }
    }

    fn advance(&mut self, byte: &mut u8) {
        match self.position {
            0 => self.descriptor_len = *byte,
            1 => self.descriptor_type = *byte,
            ENDPOINT_ADDRESS_OFFSET => self.endpoint = *byte,
            ENDPOINT_INTERVAL_OFFSET if self.descriptor_type == ENDPOINT_DESCRIPTOR_TYPE => {
                if let Some(interval) =
                    speed_dependent_interval(self.face, EndpointAddress::from(self.endpoint))
                {
                    *byte = interval;
                }
            }
            _ => {}
        }
        self.position = self.position.saturating_add(1);
        if self.position >= self.descriptor_len {
            self.position = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::{Mode, PollingRate, Speed};

    fn rewrite<const LEN: usize>(face: Face, mut descriptor: [u8; LEN]) -> [u8; LEN] {
        let mut rewrite = IntervalRewrite::new(face);
        for byte in &mut descriptor {
            rewrite.advance(byte);
        }
        descriptor
    }

    #[test]
    fn only_the_gamepad_endpoint_intervals_follow_the_speed() {
        let descriptor = [
            9, 4, 0, 0, 1, 3, 0, 0, 0, 7, 5, 0x81, 3, 32, 0, 4, 7, 5, 0x82, 3, 28, 0, 1,
        ];
        let face = Face {
            mode: Mode::Gamepad,
            speed: Speed::Full,
            gamepad_polling: PollingRate::Every1Millisecond,
            ..Face::default()
        };
        let rewritten = rewrite(face, descriptor);
        assert_eq!(rewritten[15], 1);
        assert_eq!(rewritten[22], 1);
        assert_eq!(rewritten[..15], descriptor[..15]);
        let high_speed = rewrite(
            Face {
                speed: Speed::High,
                ..face
            },
            descriptor,
        );
        assert_eq!(high_speed, descriptor);
    }

    #[test]
    fn a_zero_length_descriptor_does_not_stall_the_walk() {
        let face = Face {
            mode: Mode::Xinput,
            speed: Speed::Full,
            gamepad_polling: PollingRate::Every4Milliseconds,
            ..Face::default()
        };
        let rewritten = rewrite(face, [0, 7, 5, 0x01, 3, 32, 0, 6]);
        assert_eq!(rewritten, [0, 7, 5, 0x01, 3, 32, 0, 4]);
    }
}
