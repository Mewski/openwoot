use core::{future::poll_fn, hint::spin_loop, task::Poll};

use embassy_futures::yield_now;
use embassy_sync::waitqueue::AtomicWaker;
use embedded_hal_async::i2c::NoAcknowledgeSource;

use super::{
    Error,
    registers::{
        CTL0_STOP, CTL1_DMAON, CTL1_ERRIE, CTL1_EVIE, STAT1_MASTER, modify_ctl1, registers,
    },
};
use crate::{
    dma,
    interrupt::{Handler, typelevel},
    pac,
    register::{read_bits, set_bits},
};

pub(super) type Channel = dma::Channel<0, 5>;

const ABORT_SPINS: u32 = 10_000;

static WAKER: AtomicWaker = AtomicWaker::new();

#[derive(Debug)]
pub struct InterruptHandler;

pub(super) struct Transfer<'t> {
    i2c: &'t pac::I2c0,
    finished: bool,
}

impl<'t> Transfer<'t> {
    pub(super) const fn new(i2c: &'t pac::I2c0) -> Self {
        Self {
            i2c,
            finished: false,
        }
    }

    pub(super) async fn event(
        &mut self,
        flag: u32,
        source: NoAcknowledgeSource,
    ) -> Result<(), Error> {
        let i2c = self.i2c;
        poll_fn(|context| {
            WAKER.register(context.waker());
            let status = read_bits(i2c.stat0());
            if let Some(error) = Error::from_status(status, source) {
                return Poll::Ready(Err(error));
            }
            if status & flag != 0 {
                return Poll::Ready(Ok(()));
            }
            critical_section::with(|section| {
                modify_ctl1(section, i2c, |bits| bits | CTL1_EVIE | CTL1_ERRIE);
            });
            Poll::Pending
        })
        .await
    }

    pub(super) async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let channel = Channel::new();
        channel.clear_control(dma::ENABLE);
        channel.clear_flags();
        channel.set_peripheral_address(self.i2c.data().as_ptr() as u32);
        channel.set_memory_address(bytes.as_ptr() as u32);
        channel.set_count(u32::try_from(bytes.len()).unwrap_or(0));
        channel.write_control(
            dma::MEMORY_TO_PERIPHERAL
                | dma::MEMORY_INCREMENT
                | dma::PRIORITY_ULTRA_HIGH
                | dma::INTERRUPTS
                | dma::ENABLE,
        );
        let i2c = self.i2c;
        poll_fn(|context| {
            WAKER.register(context.waker());
            if let Some(error) =
                Error::from_status(read_bits(i2c.stat0()), NoAcknowledgeSource::Data)
            {
                return Poll::Ready(Err(error));
            }
            let flags = channel.flags();
            if flags & dma::FLAG_TRANSFER_ERROR != 0 {
                return Poll::Ready(Err(Error::Dma));
            }
            if flags & dma::FLAG_FULL_TRANSFER != 0 {
                return Poll::Ready(Ok(()));
            }
            critical_section::with(|section| {
                channel.set_control(dma::INTERRUPTS);
                modify_ctl1(section, i2c, |bits| bits | CTL1_ERRIE);
            });
            Poll::Pending
        })
        .await
    }

    pub(super) async fn stop(&mut self) -> Result<(), Error> {
        let i2c = self.i2c;
        set_bits(i2c.ctl0(), CTL0_STOP);
        self.release_dma();
        self.finished = true;
        while read_bits(i2c.ctl0()) & CTL0_STOP != 0 {
            yield_now().await;
        }
        Ok(())
    }

    fn release_dma(&self) {
        let channel = Channel::new();
        critical_section::with(|section| {
            modify_ctl1(section, self.i2c, |bits| {
                bits & !(CTL1_DMAON | CTL1_EVIE | CTL1_ERRIE)
            });
            channel.clear_control(dma::ENABLE | dma::INTERRUPTS);
        });
        let mut spins = 0;
        while channel.control() & dma::ENABLE != 0 && spins < ABORT_SPINS {
            spins += 1;
            spin_loop();
        }
        channel.clear_flags();
    }
}

impl Drop for Transfer<'_> {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.release_dma();
        if read_bits(self.i2c.stat1()) & STAT1_MASTER != 0 {
            set_bits(self.i2c.ctl0(), CTL0_STOP);
        }
    }
}

impl Handler<typelevel::I2C0_EV> for InterruptHandler {
    // SAFETY: clearing the event interrupt enable and waking has no requirement on the caller.
    unsafe fn on_interrupt() {
        critical_section::with(|section| {
            modify_ctl1(section, registers(), |bits| bits & !CTL1_EVIE);
        });
        WAKER.wake();
    }
}

impl Handler<typelevel::I2C0_ER> for InterruptHandler {
    // SAFETY: clearing the error interrupt enable and waking has no requirement on the caller.
    unsafe fn on_interrupt() {
        critical_section::with(|section| {
            modify_ctl1(section, registers(), |bits| {
                bits & !(CTL1_ERRIE | CTL1_EVIE)
            });
        });
        WAKER.wake();
    }
}

impl Handler<typelevel::DMA0_Channel5> for InterruptHandler {
    // SAFETY: the handler only clears the interrupt enables of channel 5, which only the LED
    // bus uses.
    unsafe fn on_interrupt() {
        Channel::new().clear_control(dma::INTERRUPTS);
        WAKER.wake();
    }
}
