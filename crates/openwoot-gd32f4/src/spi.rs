use core::{
    convert::Infallible,
    future::poll_fn,
    hint::spin_loop,
    sync::atomic::{AtomicU8, Ordering, compiler_fence},
    task::Poll,
};

use embassy_sync::waitqueue::AtomicWaker;
use embedded_hal::{
    digital::OutputPin,
    spi::{ErrorKind, ErrorType},
};
use embedded_hal_async::spi::{Operation, SpiBus, SpiDevice};

use crate::{
    gpio::{OutputType, Pin, Pull, Speed},
    interrupt::{self, Binding, Handler, Priority, typelevel},
    pac,
    rcu::{self, Hertz},
    register::{clear_bits, modify_bits, read_bits, set_bits, write_bits},
};

const APB2EN_SPI0EN: u32 = 1 << 12;
const AHB1EN_DMA1EN: u32 = 1 << 22;
const CTL0_MASTER: u32 = 1 << 2;
const CTL0_SPIEN: u32 = 1 << 6;
const CTL0_SWNSS: u32 = 1 << 8;
const CTL0_SWNSSEN: u32 = 1 << 9;
const CTL0_KEPT: u32 = 0x3040;
const CTL1_DMAREN: u32 = 1 << 0;
const CTL1_DMATEN: u32 = 1 << 1;
const STAT_RBNE: u32 = 1 << 0;
const STAT_TRANS: u32 = 1 << 7;
const I2SCTL_I2SSEL: u32 = 1 << 11;
const DMA_CHEN: u32 = 1 << 0;
const DMA_SDEIE: u32 = 1 << 1;
const DMA_TAEIE: u32 = 1 << 2;
const DMA_FTFIE: u32 = 1 << 4;
const DMA_INTERRUPTS: u32 = DMA_SDEIE | DMA_TAEIE | DMA_FTFIE;
const DMA_MEMORY_TO_PERIPHERAL: u32 = 0b01 << 6;
const DMA_MNAGA: u32 = 1 << 10;
const DMA_PRIORITY_HIGH: u32 = 0b10 << 16;
const DMA_SUBPERIPHERAL_SPI0: u32 = 3 << 25;
const FLAG_SDE: u32 = 1 << 2;
const FLAG_TAE: u32 = 1 << 3;
const FLAG_FTF: u32 = 1 << 5;
const FLAGS_ALL: u32 = 0b11_1101;
const RECEIVE_FLAG_SHIFT: u32 = 16;
const TRANSMIT_FLAG_SHIFT: u32 = 22;
const MAX_TRANSFER: usize = 0xFFFF;
const IDLE_WORD: u8 = 0x00;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

static RECEIVED: AtomicWaker = AtomicWaker::new();
static DISCARDED: AtomicU8 = AtomicU8::new(0);
static IDLE: u8 = IDLE_WORD;

pub trait ClockPin {
    fn into_spi(self);
}

pub trait DataOutPin {
    fn into_spi(self);
}

pub trait DataInPin {
    fn into_spi(self);
}

#[derive(Debug)]
pub struct Spi {
    spi: pac::Spi0,
}

#[derive(Debug)]
pub struct Device<ChipSelect> {
    bus: Spi,
    chip_select: ChipSelect,
    core_clock: Hertz,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Transfer,
}

#[derive(Debug)]
pub struct InterruptHandler;

#[derive(Clone, Copy)]
enum Source {
    Bytes(*const u8),
    Idle,
}

#[derive(Clone, Copy)]
enum Sink {
    Bytes(*mut u8),
    Discard,
}

struct Transfer<'s> {
    spi: &'s pac::Spi0,
}

struct Selected<'c, ChipSelect: OutputPin<Error = Infallible>> {
    chip_select: &'c mut ChipSelect,
}

macro_rules! spi_pins {
    ($pin_trait:ident: $(($port:literal, $index:literal)),+) => {
        $(
            impl<Mode> $pin_trait for Pin<$port, $index, Mode> {
                fn into_spi(self) {
                    let _ = self.into_alternate::<5>(OutputType::PushPull, Speed::High, Pull::None);
                }
            }
        )+
    };
}

spi_pins!(ClockPin: ('A', 5), ('B', 3));
spi_pins!(DataOutPin: ('A', 7), ('B', 5));
spi_pins!(DataInPin: ('A', 6), ('B', 4));

impl Spi {
    #[must_use]
    pub fn new(
        spi: pac::Spi0,
        clock: impl ClockPin,
        data_out: impl DataOutPin,
        data_in: impl DataInPin,
        _irq: impl Binding<typelevel::DMA1_Channel2, InterruptHandler>,
    ) -> Self {
        rcu::with_registers(|rcu| {
            set_bits(rcu.apb2en(), APB2EN_SPI0EN);
            set_bits(rcu.ahb1en(), AHB1EN_DMA1EN);
        });
        clock.into_spi();
        data_out.into_spi();
        data_in.into_spi();
        modify_bits(spi.ctl0(), control);
        clear_bits(spi.i2sctl(), I2SCTL_I2SSEL);
        set_bits(spi.ctl0(), CTL0_SPIEN);
        interrupt::enable(pac::Interrupt::DMA1_Channel2, Priority::HIGHEST);
        Self { spi }
    }

    async fn exchange(&mut self, source: Source, sink: Sink, len: usize) -> Result<(), Error> {
        let mut done = 0;
        while done < len {
            let chunk = (len - done).min(MAX_TRANSFER);
            let transfer =
                Transfer::start(&self.spi, source.advanced(done), sink.advanced(done), chunk);
            transfer.finished().await?;
            done += chunk;
        }
        Ok(())
    }

    fn idle(&self) {
        while read_bits(self.spi.stat()) & STAT_TRANS != 0 {
            spin_loop();
        }
    }
}

impl<ChipSelect: OutputPin<Error = Infallible>> Device<ChipSelect> {
    #[must_use]
    pub fn new(bus: Spi, mut chip_select: ChipSelect, core_clock: Hertz) -> Self {
        let Ok(()) = chip_select.set_high();
        Self {
            bus,
            chip_select,
            core_clock,
        }
    }

    pub fn release(self) -> (Spi, ChipSelect) {
        (self.bus, self.chip_select)
    }
}

impl Handler<typelevel::DMA1_Channel2> for InterruptHandler {
    // SAFETY: the handler only reads the flags of the receive channel and clears that channel's
    // interrupt enables, which no other code changes while a transfer runs.
    unsafe fn on_interrupt() {
        let dma = dma();
        if receive_flags(dma) & (FLAG_FTF | FLAG_TAE | FLAG_SDE) != 0 {
            clear_bits(dma.ch2ctl(), DMA_INTERRUPTS);
            RECEIVED.wake();
        }
    }
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl ErrorType for Spi {
    type Error = Error;
}

impl SpiBus for Spi {
    async fn read(&mut self, words: &mut [u8]) -> Result<(), Error> {
        self.exchange(Source::Idle, Sink::Bytes(words.as_mut_ptr()), words.len())
            .await
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Error> {
        self.exchange(Source::Bytes(words.as_ptr()), Sink::Discard, words.len())
            .await
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        let common = read.len().min(write.len());
        let (read_common, read_rest) = read.split_at_mut(common);
        let (write_common, write_rest) = write.split_at(common);
        self.exchange(
            Source::Bytes(write_common.as_ptr()),
            Sink::Bytes(read_common.as_mut_ptr()),
            common,
        )
        .await?;
        self.read(read_rest).await?;
        self.write(write_rest).await
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Error> {
        let len = words.len();
        let pointer = words.as_mut_ptr();
        self.exchange(Source::Bytes(pointer), Sink::Bytes(pointer), len)
            .await
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), Error>> {
        self.idle();
        core::future::ready(Ok(()))
    }
}

impl<ChipSelect: OutputPin<Error = Infallible>> ErrorType for Device<ChipSelect> {
    type Error = Error;
}

impl<ChipSelect: OutputPin<Error = Infallible>> SpiDevice for Device<ChipSelect> {
    async fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Error> {
        let selected = Selected::new(&mut self.chip_select);
        let mut result = Ok(());
        for operation in operations {
            result = match operation {
                Operation::Read(words) => self.bus.read(words).await,
                Operation::Write(words) => self.bus.write(words).await,
                Operation::Transfer(read, write) => self.bus.transfer(read, write).await,
                Operation::TransferInPlace(words) => self.bus.transfer_in_place(words).await,
                Operation::DelayNs(nanoseconds) => {
                    self.bus.idle();
                    delay(*nanoseconds, self.core_clock);
                    Ok(())
                }
            };
            if result.is_err() {
                break;
            }
        }
        self.bus.idle();
        drop(selected);
        result
    }
}

impl Source {
    fn advanced(self, offset: usize) -> Self {
        match self {
            Self::Bytes(pointer) => Self::Bytes(pointer.wrapping_add(offset)),
            Self::Idle => Self::Idle,
        }
    }

    fn address(self) -> u32 {
        match self {
            Self::Bytes(pointer) => address(pointer),
            Self::Idle => address(&raw const IDLE),
        }
    }
}

impl Sink {
    fn advanced(self, offset: usize) -> Self {
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
    fn start(spi: &'s pac::Spi0, source: Source, sink: Sink, len: usize) -> Self {
        while read_bits(spi.stat()) & STAT_RBNE != 0 {
            let _ = read_bits(spi.data());
        }
        let dma = dma();
        let count = u32::try_from(len).unwrap_or_default();
        let data = address(spi.data().as_ptr().cast_const().cast::<u8>());
        write_bits(
            dma.intc0(),
            FLAGS_ALL << RECEIVE_FLAG_SHIFT | FLAGS_ALL << TRANSMIT_FLAG_SHIFT,
        );
        write_bits(dma.ch2paddr(), data);
        write_bits(dma.ch2m0addr(), sink.address());
        write_bits(dma.ch2cnt(), count);
        write_bits(dma.ch3paddr(), data);
        write_bits(dma.ch3m0addr(), source.address());
        write_bits(dma.ch3cnt(), count);
        compiler_fence(Ordering::SeqCst);
        critical_section::with(|_| {
            write_bits(dma.ch2ctl(), receive_control(sink) | DMA_CHEN);
            write_bits(dma.ch3ctl(), transmit_control(source) | DMA_CHEN);
        });
        set_bits(spi.ctl1(), CTL1_DMAREN | CTL1_DMATEN);
        Self { spi }
    }

    async fn finished(self) -> Result<(), Error> {
        poll_fn(|context| {
            RECEIVED.register(context.waker());
            let flags = receive_flags(dma());
            if flags & (FLAG_TAE | FLAG_SDE) != 0 {
                Poll::Ready(Err(Error::Transfer))
            } else if flags & FLAG_FTF != 0 {
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
        clear_bits(self.spi.ctl1(), CTL1_DMAREN | CTL1_DMATEN);
        let dma = dma();
        critical_section::with(|_| {
            write_bits(dma.ch2ctl(), 0);
            write_bits(dma.ch3ctl(), 0);
        });
        while read_bits(dma.ch2ctl()) & DMA_CHEN != 0 || read_bits(dma.ch3ctl()) & DMA_CHEN != 0 {
            spin_loop();
        }
        write_bits(
            dma.intc0(),
            FLAGS_ALL << RECEIVE_FLAG_SHIFT | FLAGS_ALL << TRANSMIT_FLAG_SHIFT,
        );
        compiler_fence(Ordering::SeqCst);
    }
}

impl<'c, ChipSelect: OutputPin<Error = Infallible>> Selected<'c, ChipSelect> {
    fn new(chip_select: &'c mut ChipSelect) -> Self {
        let Ok(()) = chip_select.set_low();
        Self { chip_select }
    }
}

impl<ChipSelect: OutputPin<Error = Infallible>> Drop for Selected<'_, ChipSelect> {
    fn drop(&mut self) {
        let Ok(()) = self.chip_select.set_high();
    }
}

const fn control(ctl0: u32) -> u32 {
    (ctl0 & CTL0_KEPT) | CTL0_MASTER | CTL0_SWNSS | CTL0_SWNSSEN
}

const fn receive_control(sink: Sink) -> u32 {
    let increment = match sink {
        Sink::Bytes(_) => DMA_MNAGA,
        Sink::Discard => 0,
    };
    DMA_SUBPERIPHERAL_SPI0 | DMA_PRIORITY_HIGH | increment | DMA_INTERRUPTS
}

const fn transmit_control(source: Source) -> u32 {
    let increment = match source {
        Source::Bytes(_) => DMA_MNAGA,
        Source::Idle => 0,
    };
    DMA_SUBPERIPHERAL_SPI0 | DMA_PRIORITY_HIGH | DMA_MEMORY_TO_PERIPHERAL | increment
}

fn receive_flags(dma: &pac::dma0::RegisterBlock) -> u32 {
    (read_bits(dma.intf0()) >> RECEIVE_FLAG_SHIFT) & FLAGS_ALL
}

fn delay(nanoseconds: u32, core_clock: Hertz) {
    let cycles = u64::from(nanoseconds) * u64::from(core_clock.0) / NANOSECONDS_PER_SECOND;
    cortex_m::asm::delay(u32::try_from(cycles).unwrap_or(u32::MAX));
}

fn address(pointer: *const u8) -> u32 {
    u32::try_from(pointer.expose_provenance()).unwrap_or_default()
}

fn dma() -> &'static pac::dma0::RegisterBlock {
    // SAFETY: the controller block is shared with other drivers, but this driver writes only the
    // registers of channels 2 and 3 and only their bits of the write-one-to-clear flag register;
    // channel registers are changed inside critical sections, which also mask the handler.
    unsafe { &*pac::Dma1::ptr() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bus_is_a_software_selected_mode_0_master_at_half_the_bus_clock() {
        assert_eq!(control(0), 0x0304);
        assert_eq!(control(0xFFFF), 0x3344);
        assert_eq!(control(0) | CTL0_SPIEN, 0x0344);
    }

    #[test]
    fn receive_and_transmit_channels_serve_spi0_in_the_right_directions() {
        assert_eq!(
            receive_control(Sink::Bytes(core::ptr::null_mut())),
            0x0602_0416
        );
        assert_eq!(receive_control(Sink::Discard), 0x0602_0016);
        assert_eq!(
            transmit_control(Source::Bytes(core::ptr::null())),
            0x0602_0440
        );
        assert_eq!(transmit_control(Source::Idle), 0x0602_0040);
    }
}
