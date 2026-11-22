use core::{future::poll_fn, hint::spin_loop, task::Poll};

use embassy_futures::yield_now;
use embassy_sync::waitqueue::AtomicWaker;
use embassy_time::{Duration, with_timeout};
use embedded_hal_async::i2c::{
    self as hal_i2c, ErrorKind, ErrorType, NoAcknowledgeSource, Operation, SevenBitAddress,
};

use crate::{
    gpio::{OutputType, Pin, Pull, Speed},
    interrupt::{self, Binding, Handler, Priority, typelevel},
    pac,
    rcu::{self, Clocks, Hertz},
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

pub const BUS_HZ: u32 = 400_000;
pub const TRANSFER_TIMEOUT: Duration = Duration::from_millis(10);

const APB1EN_I2C0EN: u32 = 1 << 21;
const AHB1EN_DMA0EN: u32 = 1 << 21;
const OWN_ADDRESS: u32 = 0xFF;
const CTL0_I2CEN: u32 = 1 << 0;
const CTL0_SMBEN: u32 = 1 << 1;
const CTL0_START: u32 = 1 << 8;
const CTL0_STOP: u32 = 1 << 9;
const CTL0_ACKEN: u32 = 1 << 10;
const CTL0_SRESET: u32 = 1 << 15;
const CTL1_I2CCLK: u32 = 0x3F;
const CTL1_ERRIE: u32 = 1 << 8;
const CTL1_EVIE: u32 = 1 << 9;
const CTL1_DMAON: u32 = 1 << 11;
const STAT0_SBSEND: u32 = 1 << 0;
const STAT0_ADDSEND: u32 = 1 << 1;
const STAT0_BTC: u32 = 1 << 2;
const STAT0_BERR: u32 = 1 << 8;
const STAT0_LOSTARB: u32 = 1 << 9;
const STAT0_AERR: u32 = 1 << 10;
const STAT0_OUERR: u32 = 1 << 11;
const STAT0_ERRORS: u32 = STAT0_BERR | STAT0_LOSTARB | STAT0_AERR | STAT0_OUERR;
const STAT1_MASTER: u32 = 1 << 0;
const STAT1_I2CBSY: u32 = 1 << 1;
const CKCFG_DTCY: u32 = 1 << 14;
const CKCFG_FAST: u32 = 1 << 15;
const CKCFG_CLKC: u32 = 0xFFF;
const CLOCK_MAX_MHZ: u32 = 60;
const DMA_CHEN: u32 = 1 << 0;
const DMA_SDEIE: u32 = 1 << 1;
const DMA_TAEIE: u32 = 1 << 2;
const DMA_FTFIE: u32 = 1 << 4;
const DMA_INTERRUPTS: u32 = DMA_SDEIE | DMA_TAEIE | DMA_FTFIE;
const DMA_MEMORY_TO_PERIPHERAL: u32 = 0b01 << 6;
const DMA_MNAGA: u32 = 1 << 10;
const DMA_PRIORITY_ULTRA_HIGH: u32 = 0b11 << 16;
const DMA_SUBPERIPHERAL_I2C0_TX: u32 = 1 << 25;
const DMA_FIFO_RESET: u32 = 0x21;
const FLAG_SHIFT: u32 = 16;
const FLAG_SDE: u32 = 1 << 2;
const FLAG_TAE: u32 = 1 << 3;
const FLAG_FTF: u32 = 1 << 5;
const FLAGS_ALL: u32 = 0b11_1101;
const MAX_TRANSFER: usize = 0xFFFF;
const ABORT_SPINS: u32 = 10_000;

static WAKER: AtomicWaker = AtomicWaker::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the LED bus did not finish a transfer within 10 ms")]
    Timeout,
    #[error("the LED bus saw a misplaced start or stop condition")]
    Bus,
    #[error("the LED bus lost arbitration")]
    ArbitrationLoss,
    #[error("the device did not acknowledge its {0:?}")]
    NoAcknowledge(NoAcknowledgeSource),
    #[error("the LED bus over- or underran")]
    Overrun,
    #[error("the LED bus DMA channel reported a transfer error")]
    Dma,
    #[error("the LED bus only writes")]
    Read,
    #[error("a transfer of {0} bytes exceeds 65535 bytes")]
    TooLong(usize),
    #[error("a transaction of more than 4 writes")]
    TooManyWrites,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    pub clock_mhz: u32,
    pub rise_time: u32,
    pub clock_configuration: u32,
}

#[must_use]
pub const fn timing(apb1: Hertz) -> Timing {
    let mut clock_mhz = apb1.0 / 1_000_000;
    if clock_mhz > CLOCK_MAX_MHZ {
        clock_mhz = CLOCK_MAX_MHZ;
    }
    let mut divider = apb1.0 / (BUS_HZ * 25);
    if divider & CKCFG_CLKC == 0 {
        divider |= 1;
    }
    Timing {
        clock_mhz,
        rise_time: clock_mhz * 300 / 1000 + 1,
        clock_configuration: CKCFG_FAST | CKCFG_DTCY | (divider & CKCFG_CLKC),
    }
}

pub trait ClockPin {
    fn into_i2c(self);
}

pub trait DataPin {
    fn into_i2c(self);
}

macro_rules! i2c_pins {
    ($pin_trait:ident: $(($port:literal, $index:literal)),+) => {
        $(
            impl<Mode> $pin_trait for Pin<$port, $index, Mode> {
                fn into_i2c(self) {
                    let _ = self.into_alternate::<4>(OutputType::OpenDrain, Speed::High, Pull::None);
                }
            }
        )+
    };
}

i2c_pins!(ClockPin: ('B', 6), ('B', 8));
i2c_pins!(DataPin: ('B', 7), ('B', 9));

#[derive(Debug)]
pub struct InterruptHandler;

#[derive(Debug)]
pub struct I2c {
    i2c: pac::I2c0,
    timing: Timing,
}

struct Transfer<'t> {
    i2c: &'t pac::I2c0,
    finished: bool,
}

impl I2c {
    #[must_use]
    pub fn new(
        i2c: pac::I2c0,
        clock: impl ClockPin,
        data: impl DataPin,
        _irqs: impl Binding<typelevel::I2C0_EV, InterruptHandler>
        + Binding<typelevel::I2C0_ER, InterruptHandler>
        + Binding<typelevel::DMA0_Channel6, InterruptHandler>,
        clocks: &Clocks,
    ) -> Self {
        rcu::with_registers(|rcu| {
            set_bits(rcu.apb1en(), APB1EN_I2C0EN);
            set_bits(rcu.ahb1en(), AHB1EN_DMA0EN);
        });
        clock.into_i2c();
        data.into_i2c();
        let bus = Self {
            i2c,
            timing: timing(clocks.apb1),
        };
        bus.configure();
        for irq in [
            pac::Interrupt::I2C0_EV,
            pac::Interrupt::I2C0_ER,
            pac::Interrupt::DMA0_Channel6,
        ] {
            interrupt::enable(irq, Priority::HIGHEST);
        }
        bus
    }

    fn configure(&self) {
        let i2c = &self.i2c;
        modify_bits(i2c.ctl1(), |bits| {
            (bits & !CTL1_I2CCLK) | self.timing.clock_mhz
        });
        write_bits(i2c.rt(), self.timing.rise_time);
        write_bits(i2c.ckcfg(), self.timing.clock_configuration);
        write_bits(i2c.saddr0(), OWN_ADDRESS);
        clear_bits(i2c.ctl0(), CTL0_SMBEN);
        set_bits(i2c.ctl0(), CTL0_I2CEN);
        set_bits(i2c.ctl0(), CTL0_ACKEN);
    }

    fn recover(&self) {
        let i2c = &self.i2c;
        set_bits(i2c.ctl0(), CTL0_SRESET);
        clear_bits(i2c.ctl0(), CTL0_SRESET);
        write_bits(dma().intc1(), FLAGS_ALL << FLAG_SHIFT);
        self.configure();
    }

    async fn write_all(&self, address: SevenBitAddress, chunks: &[&[u8]]) -> Result<(), Error> {
        let i2c = &self.i2c;
        while read_bits(i2c.stat1()) & STAT1_I2CBSY != 0 {
            yield_now().await;
        }
        modify_bits(i2c.stat0(), |bits| bits & !STAT0_ERRORS);
        let mut transfer = Transfer {
            i2c,
            finished: false,
        };
        set_bits(i2c.ctl0(), CTL0_START);
        transfer
            .event(STAT0_SBSEND, NoAcknowledgeSource::Address)
            .await?;
        write_bits(i2c.data(), u32::from(address) << 1);
        transfer
            .event(STAT0_ADDSEND, NoAcknowledgeSource::Address)
            .await?;
        let _ = read_bits(i2c.stat0());
        let _ = read_bits(i2c.stat1());
        set_bits(i2c.ctl1(), CTL1_DMAON);
        for chunk in chunks.iter().filter(|chunk| !chunk.is_empty()) {
            transfer.send(chunk).await?;
        }
        transfer.event(STAT0_BTC, NoAcknowledgeSource::Data).await?;
        transfer.stop().await;
        Ok(())
    }
}

impl Transfer<'_> {
    async fn event(&mut self, flag: u32, source: NoAcknowledgeSource) -> Result<(), Error> {
        let i2c = self.i2c;
        poll_fn(|context| {
            WAKER.register(context.waker());
            let status = read_bits(i2c.stat0());
            if let Some(error) = error_of(status, source) {
                return Poll::Ready(Err(error));
            }
            if status & flag != 0 {
                return Poll::Ready(Ok(()));
            }
            critical_section::with(|_| set_bits(i2c.ctl1(), CTL1_EVIE | CTL1_ERRIE));
            Poll::Pending
        })
        .await
    }

    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let dma = dma();
        clear_bits(dma.ch6ctl(), DMA_CHEN);
        write_bits(dma.intc1(), FLAGS_ALL << FLAG_SHIFT);
        write_bits(dma.ch6paddr(), self.i2c.data().as_ptr() as u32);
        write_bits(dma.ch6m0addr(), bytes.as_ptr() as u32);
        write_bits(dma.ch6cnt(), u32::try_from(bytes.len()).unwrap_or(0));
        write_bits(dma.ch6fctl(), DMA_FIFO_RESET);
        write_bits(
            dma.ch6ctl(),
            DMA_MEMORY_TO_PERIPHERAL
                | DMA_MNAGA
                | DMA_PRIORITY_ULTRA_HIGH
                | DMA_SUBPERIPHERAL_I2C0_TX
                | DMA_INTERRUPTS
                | DMA_CHEN,
        );
        let i2c = self.i2c;
        poll_fn(|context| {
            WAKER.register(context.waker());
            if let Some(error) = error_of(read_bits(i2c.stat0()), NoAcknowledgeSource::Data) {
                return Poll::Ready(Err(error));
            }
            let flags = read_bits(dma.intf1()) >> FLAG_SHIFT;
            if flags & (FLAG_TAE | FLAG_SDE) != 0 {
                return Poll::Ready(Err(Error::Dma));
            }
            if flags & FLAG_FTF != 0 {
                return Poll::Ready(Ok(()));
            }
            critical_section::with(|_| {
                set_bits(dma.ch6ctl(), DMA_INTERRUPTS);
                set_bits(i2c.ctl1(), CTL1_ERRIE);
            });
            Poll::Pending
        })
        .await
    }

    async fn stop(&mut self) {
        let i2c = self.i2c;
        set_bits(i2c.ctl0(), CTL0_STOP);
        self.release_dma();
        self.finished = true;
        while read_bits(i2c.ctl0()) & CTL0_STOP != 0 {
            yield_now().await;
        }
    }

    fn release_dma(&self) {
        let dma = dma();
        critical_section::with(|_| {
            clear_bits(self.i2c.ctl1(), CTL1_DMAON | CTL1_EVIE | CTL1_ERRIE);
            clear_bits(dma.ch6ctl(), DMA_CHEN | DMA_INTERRUPTS);
        });
        let mut spins = 0;
        while read_bits(dma.ch6ctl()) & DMA_CHEN != 0 && spins < ABORT_SPINS {
            spins += 1;
            spin_loop();
        }
        write_bits(dma.intc1(), FLAGS_ALL << FLAG_SHIFT);
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

fn error_of(status: u32, source: NoAcknowledgeSource) -> Option<Error> {
    if status & STAT0_AERR != 0 {
        Some(Error::NoAcknowledge(source))
    } else if status & STAT0_LOSTARB != 0 {
        Some(Error::ArbitrationLoss)
    } else if status & STAT0_BERR != 0 {
        Some(Error::Bus)
    } else if status & STAT0_OUERR != 0 {
        Some(Error::Overrun)
    } else {
        None
    }
}

fn dma() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: the LED bus uses only channel 6 of DMA0: its control, count, address and FIFO
    // registers, and its own flag bits through the write-one-to-clear register, so it never
    // changes what another user of DMA0 owns.
    unsafe { &*pac::Dma0::ptr() }
}

fn registers() -> &'static pac::i2c0::RegisterBlock {
    // SAFETY: an `I2c` owns the I2C0 peripheral; the interrupt handlers only clear its interrupt
    // enable bits and wake the waiting transfer.
    unsafe { &*pac::I2c0::ptr() }
}

impl Handler<typelevel::I2C0_EV> for InterruptHandler {
    // SAFETY: clearing the event interrupt enable and waking has no requirement on the caller.
    unsafe fn on_interrupt() {
        clear_bits(registers().ctl1(), CTL1_EVIE);
        WAKER.wake();
    }
}

impl Handler<typelevel::I2C0_ER> for InterruptHandler {
    // SAFETY: clearing the error interrupt enable and waking has no requirement on the caller.
    unsafe fn on_interrupt() {
        clear_bits(registers().ctl1(), CTL1_ERRIE | CTL1_EVIE);
        WAKER.wake();
    }
}

impl Handler<typelevel::DMA0_Channel6> for InterruptHandler {
    // SAFETY: the handler only clears the interrupt enables of channel 6, which only the LED
    // bus uses.
    unsafe fn on_interrupt() {
        clear_bits(dma().ch6ctl(), DMA_INTERRUPTS);
        WAKER.wake();
    }
}

impl hal_i2c::Error for Error {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Bus => ErrorKind::Bus,
            Self::ArbitrationLoss => ErrorKind::ArbitrationLoss,
            Self::NoAcknowledge(source) => ErrorKind::NoAcknowledge(*source),
            Self::Overrun => ErrorKind::Overrun,
            Self::Timeout | Self::Dma | Self::Read | Self::TooLong(_) | Self::TooManyWrites => {
                ErrorKind::Other
            }
        }
    }
}

impl ErrorType for I2c {
    type Error = Error;
}

impl hal_i2c::I2c for I2c {
    async fn transaction(
        &mut self,
        address: SevenBitAddress,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        let mut chunks: [&[u8]; 4] = [&[]; 4];
        let mut count = 0;
        for operation in operations.iter() {
            match operation {
                Operation::Read(_) => return Err(Error::Read),
                Operation::Write(bytes) => {
                    if bytes.len() > MAX_TRANSFER {
                        return Err(Error::TooLong(bytes.len()));
                    }
                    let Some(slot) = chunks.get_mut(count) else {
                        return Err(Error::TooManyWrites);
                    };
                    *slot = bytes;
                    count += 1;
                }
            }
        }
        let result = match with_timeout(TRANSFER_TIMEOUT, self.write_all(address, &chunks)).await {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout),
        };
        if result.is_err() {
            self.recover();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_mode_timing_matches_the_reference_set_up() {
        assert_eq!(
            timing(Hertz::from_mhz(50)),
            Timing {
                clock_mhz: 50,
                rise_time: 16,
                clock_configuration: 0xC005,
            }
        );
        assert_eq!(timing(Hertz::from_mhz(100)).clock_mhz, 60);
    }

    #[test]
    fn status_errors_map_to_their_kinds() {
        assert_eq!(error_of(0, NoAcknowledgeSource::Data), None);
        assert_eq!(
            error_of(STAT0_AERR | STAT0_BTC, NoAcknowledgeSource::Address),
            Some(Error::NoAcknowledge(NoAcknowledgeSource::Address))
        );
        assert_eq!(hal_i2c::Error::kind(&Error::Timeout), ErrorKind::Other);
        assert_eq!(
            error_of(STAT0_LOSTARB, NoAcknowledgeSource::Data),
            Some(Error::ArbitrationLoss)
        );
    }
}
