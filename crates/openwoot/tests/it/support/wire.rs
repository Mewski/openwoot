use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    future::{Future, poll_fn, ready},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use embassy_usb_driver as driver;
use embassy_usb_driver::{
    Direction, EndpointAddress, EndpointAllocError, EndpointError, EndpointInfo, EndpointType,
    Event, Unsupported,
};

const CONTROL_PACKET_SIZE: usize = 64;
const POLLS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Accepted,
    Rejected,
    Address(u8),
}

#[derive(Debug, Default)]
struct State {
    events: VecDeque<Event>,
    setups: VecDeque<[u8; 8]>,
    control_out: VecDeque<Vec<u8>>,
    control_in: Vec<u8>,
    outcome: Option<Outcome>,
    sent: HashMap<u8, Vec<Vec<u8>>>,
    received: HashMap<u8, VecDeque<Vec<u8>>>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Host(Rc<RefCell<State>>);

pub(crate) struct Driver {
    host: Host,
    allocated_in: u16,
    allocated_out: u16,
}

pub(crate) struct Endpoint {
    host: Host,
    info: EndpointInfo,
}

pub(crate) struct Bus {
    host: Host,
}

pub(crate) struct ControlPipe {
    host: Host,
}

impl Host {
    #[must_use]
    pub(crate) fn driver(&self) -> Driver {
        Driver {
            host: self.clone(),
            allocated_in: 1,
            allocated_out: 1,
        }
    }

    pub(crate) fn attach<F: Future>(&self, device: Pin<&mut F>) {
        self.attach_with(device, &[Event::PowerDetected, Event::Reset]);
    }

    pub(crate) fn attach_after_idle_bus<F: Future>(&self, device: Pin<&mut F>) {
        self.attach_with(
            device,
            &[Event::PowerDetected, Event::Suspend, Event::Reset],
        );
    }

    fn attach_with<F: Future>(&self, device: Pin<&mut F>, events: &[Event]) {
        self.0.borrow_mut().events.extend(events.iter().copied());
        settle(device);
    }

    pub(crate) fn control_in<F: Future>(
        &self,
        device: Pin<&mut F>,
        setup: [u8; 8],
    ) -> Result<Vec<u8>, Outcome> {
        self.start(setup, &[]);
        settle(device);
        let mut state = self.0.borrow_mut();
        match state.outcome {
            Some(outcome) => Err(outcome),
            None => Ok(std::mem::take(&mut state.control_in)),
        }
    }

    #[must_use]
    pub(crate) fn control_out<F: Future>(
        &self,
        device: Pin<&mut F>,
        setup: [u8; 8],
        data: &[u8],
    ) -> Option<Outcome> {
        self.start(setup, data);
        settle(device);
        self.0.borrow().outcome
    }

    pub(crate) fn send_packets(&self, endpoint: u8, transfer: &[u8], packet_size: usize) {
        let mut state = self.0.borrow_mut();
        let queue = state.received.entry(endpoint).or_default();
        queue.extend(transfer.chunks(packet_size).map(<[u8]>::to_vec));
        if transfer.len().is_multiple_of(packet_size) {
            queue.push_back(Vec::new());
        }
    }

    #[must_use]
    pub(crate) fn take_sent(&self, endpoint: u8) -> Vec<Vec<u8>> {
        self.0
            .borrow_mut()
            .sent
            .remove(&endpoint)
            .unwrap_or_default()
    }

    fn start(&self, setup: [u8; 8], data: &[u8]) {
        let mut state = self.0.borrow_mut();
        state.setups.push_back(setup);
        state.control_out = data
            .chunks(CONTROL_PACKET_SIZE)
            .map(<[u8]>::to_vec)
            .collect();
        state.control_in.clear();
        state.outcome = None;
    }
}

#[must_use]
pub(crate) fn poll<F: Future>(mut future: Pin<&mut F>) -> Option<F::Output> {
    let mut context = Context::from_waker(Waker::noop());
    (0..POLLS).find_map(|_| match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    })
}

fn settle<F: Future>(mut future: Pin<&mut F>) {
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..POLLS {
        if future.as_mut().poll(&mut context).is_ready() {
            break;
        }
    }
}

#[must_use]
pub(crate) fn setup(request_type: u8, request: u8, value: u16, index: u16, length: u16) -> [u8; 8] {
    let [value_low, value_high] = value.to_le_bytes();
    let [index_low, index_high] = index.to_le_bytes();
    let [length_low, length_high] = length.to_le_bytes();
    [
        request_type,
        request,
        value_low,
        value_high,
        index_low,
        index_high,
        length_low,
        length_high,
    ]
}

impl Driver {
    fn allocate(
        &mut self,
        direction: Direction,
        ep_type: EndpointType,
        address: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Endpoint, EndpointAllocError> {
        let allocated = match direction {
            Direction::In => &mut self.allocated_in,
            Direction::Out => &mut self.allocated_out,
        };
        let index = match address {
            Some(address) if *allocated & (1 << address.index()) == 0 => address.index(),
            Some(_) => return Err(EndpointAllocError),
            None => (1..16)
                .find(|index| *allocated & (1 << index) == 0)
                .ok_or(EndpointAllocError)?,
        };
        *allocated |= 1 << index;
        Ok(Endpoint {
            host: self.host.clone(),
            info: EndpointInfo {
                addr: EndpointAddress::from_parts(index, direction),
                ep_type,
                max_packet_size,
                interval_ms,
            },
        })
    }
}

impl driver::Driver<'_> for Driver {
    type Bus = Bus;
    type ControlPipe = ControlPipe;
    type EndpointIn = Endpoint;
    type EndpointOut = Endpoint;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Endpoint, EndpointAllocError> {
        self.allocate(
            Direction::Out,
            ep_type,
            ep_addr,
            max_packet_size,
            interval_ms,
        )
    }

    fn alloc_endpoint_in(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Endpoint, EndpointAllocError> {
        self.allocate(
            Direction::In,
            ep_type,
            ep_addr,
            max_packet_size,
            interval_ms,
        )
    }

    fn start(self, _control_max_packet_size: u16) -> (Bus, ControlPipe) {
        (
            Bus {
                host: self.host.clone(),
            },
            ControlPipe { host: self.host },
        )
    }
}

impl driver::Bus for Bus {
    async fn enable(&mut self) {}

    async fn disable(&mut self) {}

    async fn poll(&mut self) -> Event {
        poll_fn(|_| {
            self.host
                .0
                .borrow_mut()
                .events
                .pop_front()
                .map_or(Poll::Pending, Poll::Ready)
        })
        .await
    }

    fn endpoint_set_enabled(&mut self, _ep_addr: EndpointAddress, _enabled: bool) {}

    fn endpoint_set_stalled(&mut self, _ep_addr: EndpointAddress, _stalled: bool) {}

    fn endpoint_is_stalled(&mut self, _ep_addr: EndpointAddress) -> bool {
        false
    }

    fn remote_wakeup(&mut self) -> impl Future<Output = Result<(), Unsupported>> {
        ready(Ok(()))
    }
}

impl driver::Endpoint for Endpoint {
    fn info(&self) -> &EndpointInfo {
        &self.info
    }

    async fn wait_enabled(&mut self) {}
}

impl driver::EndpointIn for Endpoint {
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = Result<(), EndpointError>> {
        if buf.len() > usize::from(self.info.max_packet_size) {
            return ready(Err(EndpointError::BufferOverflow));
        }
        self.host
            .0
            .borrow_mut()
            .sent
            .entry(self.info.addr.into())
            .or_default()
            .push(buf.to_vec());
        ready(Ok(()))
    }
}

impl driver::EndpointOut for Endpoint {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
        let address = u8::from(self.info.addr);
        poll_fn(|_| {
            let mut state = self.host.0.borrow_mut();
            let Some(packet) = state
                .received
                .get_mut(&address)
                .and_then(VecDeque::pop_front)
            else {
                return Poll::Pending;
            };
            Poll::Ready(
                buf.get_mut(..packet.len())
                    .map(|target| {
                        target.copy_from_slice(&packet);
                        packet.len()
                    })
                    .ok_or(EndpointError::BufferOverflow),
            )
        })
        .await
    }
}

impl driver::ControlPipe for ControlPipe {
    fn max_packet_size(&self) -> usize {
        CONTROL_PACKET_SIZE
    }

    async fn setup(&mut self) -> [u8; 8] {
        poll_fn(|_| {
            self.host
                .0
                .borrow_mut()
                .setups
                .pop_front()
                .map_or(Poll::Pending, Poll::Ready)
        })
        .await
    }

    async fn data_out(
        &mut self,
        buf: &mut [u8],
        _first: bool,
        _last: bool,
    ) -> Result<usize, EndpointError> {
        poll_fn(|_| {
            let Some(packet) = self.host.0.borrow_mut().control_out.pop_front() else {
                return Poll::Pending;
            };
            let len = packet.len().min(buf.len());
            if let (Some(target), Some(source)) = (buf.get_mut(..len), packet.get(..len)) {
                target.copy_from_slice(source);
            }
            Poll::Ready(Ok(len))
        })
        .await
    }

    fn data_in(
        &mut self,
        data: &[u8],
        _first: bool,
        _last: bool,
    ) -> impl Future<Output = Result<(), EndpointError>> {
        self.host.0.borrow_mut().control_in.extend_from_slice(data);
        ready(Ok(()))
    }

    async fn accept(&mut self) {
        self.host.0.borrow_mut().outcome = Some(Outcome::Accepted);
    }

    async fn reject(&mut self) {
        self.host.0.borrow_mut().outcome = Some(Outcome::Rejected);
    }

    async fn accept_set_address(&mut self, addr: u8) {
        self.host.0.borrow_mut().outcome = Some(Outcome::Address(addr));
    }
}
