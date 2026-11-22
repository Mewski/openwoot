use core::{future::poll_fn, marker::PhantomData, task::Poll};

use embassy_usb_driver::{
    Direction, Endpoint as EndpointTrait, EndpointError, EndpointIn, EndpointInfo, EndpointOut,
};

use super::{
    endpoint_register::{Command, Status, Toggle},
    events::EVENTS,
    packet_memory::{self, Buffer},
    registers,
};

#[derive(Debug)]
pub enum In {}

#[derive(Debug)]
pub enum Out {}

pub(super) trait DirectionMarker {
    const DIRECTION: Direction;
}

impl DirectionMarker for In {
    const DIRECTION: Direction = Direction::In;
}

impl DirectionMarker for Out {
    const DIRECTION: Direction = Direction::Out;
}

#[derive(Debug)]
pub struct Endpoint<D> {
    info: EndpointInfo,
    buffer: Buffer,
    direction: PhantomData<D>,
}

impl<D> Endpoint<D> {
    pub(super) const fn new(info: EndpointInfo, buffer: Buffer) -> Self {
        Self {
            info,
            buffer,
            direction: PhantomData,
        }
    }

    fn index(&self) -> usize {
        self.info.addr.index()
    }
}

impl Endpoint<In> {
    pub(super) fn load(&self, data: &[u8]) -> Result<(), EndpointError> {
        let len = self
            .buffer
            .write(data, registers::store)
            .ok_or(EndpointError::BufferOverflow)?;
        registers::store(packet_memory::transmit_count_slot(self.index()), len);
        Ok(())
    }
}

impl Endpoint<Out> {
    pub(super) fn unload(&self, target: &mut [u8]) -> Result<usize, EndpointError> {
        let count = registers::load(packet_memory::receive_count_slot(self.index()));
        self.buffer
            .read(count, target, registers::load)
            .ok_or(EndpointError::BufferOverflow)
    }
}

impl EndpointTrait for Endpoint<In> {
    fn info(&self) -> &EndpointInfo {
        &self.info
    }

    async fn wait_enabled(&mut self) {
        wait_for(self.index(), Direction::In, |status| {
            status != Status::Disabled
        })
        .await;
    }
}

impl EndpointTrait for Endpoint<Out> {
    fn info(&self) -> &EndpointInfo {
        &self.info
    }

    async fn wait_enabled(&mut self) {
        wait_for(self.index(), Direction::Out, |status| {
            status != Status::Disabled
        })
        .await;
    }
}

impl EndpointOut for Endpoint<Out> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
        let index = self.index();
        let status = wait_for(index, Direction::Out, idle).await;
        if status == Status::Disabled {
            return Err(EndpointError::Disabled);
        }
        let len = self.unload(buf)?;
        registers::write_endpoint(
            index,
            Command::new(self.info.ep_type, index)
                .toggle_receive(Toggle::between(Status::Nak, Status::Valid))
                .bits(),
        );
        Ok(len)
    }
}

impl EndpointIn for Endpoint<In> {
    async fn write(&mut self, buf: &[u8]) -> Result<(), EndpointError> {
        if buf.len() > usize::from(self.info.max_packet_size) {
            return Err(EndpointError::BufferOverflow);
        }
        let index = self.index();
        let status = wait_for(index, Direction::In, idle).await;
        if status == Status::Disabled {
            return Err(EndpointError::Disabled);
        }
        self.load(buf)?;
        registers::write_endpoint(
            index,
            Command::new(self.info.ep_type, index)
                .toggle_transmit(Toggle::between(Status::Nak, Status::Valid))
                .bits(),
        );
        Ok(())
    }
}

pub(super) async fn wait_for(
    index: usize,
    direction: Direction,
    ready: impl Fn(Status) -> bool,
) -> Status {
    poll_fn(|context| {
        EVENTS.register(index, direction, context);
        let status = registers::endpoint(index).status(direction);
        if ready(status) {
            Poll::Ready(status)
        } else {
            Poll::Pending
        }
    })
    .await
}

pub(super) fn idle(status: Status) -> bool {
    matches!(status, Status::Nak | Status::Disabled)
}
