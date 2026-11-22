use core::{
    future::poll_fn,
    hint::spin_loop,
    sync::atomic::{AtomicU8, Ordering, compiler_fence},
    task::Poll,
};

use embassy_sync::waitqueue::AtomicWaker;

use super::{
    Error,
    registers::{CTL1_DMAREN, CTL1_DMATEN, STAT_RBNE, STAT_TRANS, modify_ctl1},
};
use crate::{
    dma,
    interrupt::{Handler, typelevel},
    pac,
    register::read_bits,
};

pub(super) type ReceiveChannel = dma::Channel<1, 2>;
type TransmitChannel = dma::Channel<1, 3>;

const SPI0_REQUEST: u32 = dma::request(3);
const IDLE_WORD: u8 = 0x00;

static RECEIVED: AtomicWaker = AtomicWaker::new();
static DISCARDED: AtomicU8 = AtomicU8::new(0);
static IDLE: AtomicU8 = AtomicU8::new(IDLE_WORD);

#[derive(Debug)]
pub struct InterruptHandler;

#[derive(Clone, Copy)]
pub(super) enum Source {
    Bytes(*const u8),
    Idle,
}

#[derive(Clone, Copy)]
pub(super) enum Sink {
    Bytes(*mut u8),
    Discard,
}

pub(super) struct Transfer<'s> {
    spi: &'s pac::Spi0,
}

impl Source {
    pub(super) fn advanced(self, offset: usize) -> Self {
        match self {
            Self::Bytes(pointer) => Self::Bytes(pointer.wrapping_add(offset)),
            Self::Idle => Self::Idle,
        }
    }

    fn address(self) -> u32 {
        match self {
            Self::Bytes(pointer) => address(pointer),
            Self::Idle => address(IDLE.as_ptr().cast_const()),
        }
    }
}

impl Sink {
    pub(super) fn advanced(self, offset: usize) -> Self {
        match self {
            Self::Bytes(pointer) => Self::Bytes(pointer.wrapping_add(offset)),
            Self::Discard => Self::Discard,
        }
    }

    fn address(self) -> u32 {
        match self {
            Self::Bytes(pointer) => address(pointer.cast_const()),
            Self::Discard => address(DISCARDED.as_ptr().cast_const()),
        }
    }
}

impl<'s> Transfer<'s> {
    pub(super) fn start(spi: &'s pac::Spi0, source: Source, sink: Sink, len: usize) -> Self {
        while read_bits(spi.stat()) & STAT_RBNE != 0 {
            let _ = read_bits(spi.data());
        }
        let receive = ReceiveChannel::new();
        let transmit = TransmitChannel::new();
        let count = u32::try_from(len).unwrap_or_default();
        let data = address(spi.data().as_ptr().cast_const().cast::<u8>());
        receive.clear_flags();
        transmit.clear_flags();
        receive.set_peripheral_address(data);
        receive.set_memory_address(sink.address());
        receive.set_count(count);
        transmit.set_peripheral_address(data);
        transmit.set_memory_address(source.address());
        transmit.set_count(count);
        compiler_fence(Ordering::SeqCst);
        critical_section::with(|section| {
            receive.write_control(receive_control(sink) | dma::ENABLE);
            transmit.write_control(transmit_control(source) | dma::ENABLE);
            modify_ctl1(section, spi, |bits| bits | CTL1_DMAREN | CTL1_DMATEN);
        });
        Self { spi }
    }

    pub(super) async fn finished(self) -> Result<(), Error> {
        let receive = ReceiveChannel::new();
        poll_fn(|context| {
            RECEIVED.register(context.waker());
            let flags = receive.flags();
            if flags & (dma::FLAG_TRANSFER_ERROR | dma::FLAG_SINGLE_DATA_ERROR) != 0 {
                Poll::Ready(Err(Error::Transfer))
            } else if flags & dma::FLAG_FULL_TRANSFER != 0 {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

impl Drop for Transfer<'_> {
    fn drop(&mut self) {
        while read_bits(self.spi.stat()) & STAT_TRANS != 0 {
            spin_loop();
        }
        let receive = ReceiveChannel::new();
        let transmit = TransmitChannel::new();
        critical_section::with(|section| {
            modify_ctl1(section, self.spi, |bits| {
                bits & !(CTL1_DMAREN | CTL1_DMATEN)
            });
            receive.write_control(0);
            transmit.write_control(0);
        });
        while receive.control() & dma::ENABLE != 0 || transmit.control() & dma::ENABLE != 0 {
            spin_loop();
        }
        receive.clear_flags();
        transmit.clear_flags();
        compiler_fence(Ordering::SeqCst);
    }
}

impl Handler<typelevel::DMA1_Channel2> for InterruptHandler {
    // SAFETY: the handler only reads the flags of the receive channel and clears that channel's
    // interrupt enables, which no other code changes while a transfer runs.
    unsafe fn on_interrupt() {
        let receive = ReceiveChannel::new();
        if receive.flags()
            & (dma::FLAG_FULL_TRANSFER | dma::FLAG_TRANSFER_ERROR | dma::FLAG_SINGLE_DATA_ERROR)
            != 0
        {
            receive.clear_control(dma::INTERRUPTS);
            RECEIVED.wake();
        }
    }
}

const fn receive_control(sink: Sink) -> u32 {
    let increment = match sink {
        Sink::Bytes(_) => dma::MEMORY_INCREMENT,
        Sink::Discard => 0,
    };
    SPI0_REQUEST | dma::PRIORITY_LOW | increment | dma::INTERRUPTS
}

const fn transmit_control(source: Source) -> u32 {
    let increment = match source {
        Source::Bytes(_) => dma::MEMORY_INCREMENT,
        Source::Idle => 0,
    };
    SPI0_REQUEST | dma::PRIORITY_LOW | dma::MEMORY_TO_PERIPHERAL | increment
}

fn address(pointer: *const u8) -> u32 {
    u32::try_from(pointer.expose_provenance()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receive_and_transmit_channels_serve_spi0_in_the_right_directions() {
        assert_eq!(
            receive_control(Sink::Bytes(core::ptr::null_mut())),
            0x0600_0416
        );
        assert_eq!(receive_control(Sink::Discard), 0x0600_0016);
        assert_eq!(
            transmit_control(Source::Bytes(core::ptr::null())),
            0x0600_0440
        );
        assert_eq!(transmit_control(Source::Idle), 0x0600_0040);
    }
}
