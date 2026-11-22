use core::{
    fmt,
    hint::spin_loop,
    sync::atomic::{AtomicU8, Ordering},
};

use embassy_usb_driver::{
    Bus as BusTrait, Driver as DriverTrait, EndpointAddress, EndpointAllocError, EndpointType,
    Event, Speed, Unsupported,
};
use embassy_usb_synopsys_otg::{
    Bus as OtgBus, Config, ControlPipe, Driver as OtgDriver, Endpoint, In, OtgInstance, Out,
    PhyType, StateStorage, on_interrupt,
    otg_v1::{Otg, regs::Pcgcctl, vals::Dspd},
};

use crate::{
    gpio::{self, Level, Output, OutputType, Pin, Pull, Unconfigured},
    interrupt::{self, Binding, Handler, Priority, typelevel},
    pac, rcu,
    register::set_bits,
};

const ENDPOINT_COUNT: usize = 6;
const FIFO_DEPTH_WORDS: u16 = 1024;
const RECEIVE_FIFO_EXTRA_WORDS: u16 = 283;
const OUT_BUFFER_LEN: usize = 512;
const ULPI_TURNAROUND: u8 = 9;
const TIMEOUT_CALIBRATION: u8 = 7;
const ULPI_FUNCTION: u8 = 10;
const AHB1EN_USBHSEN: u32 = 1 << 29;
const AHB1EN_USBHSULPIEN: u32 = 1 << 30;
const APB2EN_SYSCFGEN: u32 = 1 << 14;
const NOT_NEGOTIATED: u8 = 0;

static STATE: StateStorage<ENDPOINT_COUNT> = StateStorage::new();
static mut OUT_BUFFER: [u8; OUT_BUFFER_LEN] = [0; OUT_BUFFER_LEN];
static NEGOTIATED: AtomicU8 = AtomicU8::new(NOT_NEGOTIATED);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeedLimit {
    High,
    Full,
}

#[derive(Debug)]
pub struct Core {
    pub global: pac::HsGlobal,
    pub host: pac::HsHost,
    pub device: pac::HsDevice,
    pub power_clock: pac::HsPwrclk,
    pub fifo: pac::HsFifo,
}

#[derive(Debug)]
pub struct UlpiPins {
    pub clock: Pin<'A', 5, Unconfigured>,
    pub direction: Pin<'C', 2, Unconfigured>,
    pub next: Pin<'C', 3, Unconfigured>,
    pub stop: Pin<'C', 0, Unconfigured>,
    pub data0: Pin<'A', 3, Unconfigured>,
    pub data1: Pin<'B', 0, Unconfigured>,
    pub data2: Pin<'B', 1, Unconfigured>,
    pub data3: Pin<'B', 10, Unconfigured>,
    pub data4: Pin<'B', 11, Unconfigured>,
    pub data5: Pin<'B', 12, Unconfigured>,
    pub data6: Pin<'B', 13, Unconfigured>,
    pub data7: Pin<'B', 5, Unconfigured>,
}

#[derive(Debug)]
pub struct InterruptHandler;

pub struct Driver {
    inner: OtgDriver<'static>,
}

pub struct Bus<'d> {
    inner: OtgBus<'d>,
    initialised: bool,
}

#[must_use]
pub fn negotiated_speed() -> Option<Speed> {
    decode(NEGOTIATED.load(Ordering::Acquire))
}

impl Handler<typelevel::USBHS> for InterruptHandler {
    // SAFETY: the handler runs only in the USBHS interrupt, as `Handler` requires.
    unsafe fn on_interrupt() {
        // SAFETY: the register block is the controller the driver owns, and `STATE` is the state
        // the driver was built with.
        unsafe { on_interrupt(registers(), &STATE.as_state()) };
    }
}

impl Driver {
    #[must_use]
    pub fn new<const PORT: char, const INDEX: u8>(
        _core: Core,
        pins: UlpiPins,
        mut phy_reset: Pin<PORT, INDEX, Output>,
        _interrupt: impl Binding<typelevel::USBHS, InterruptHandler>,
        limit: SpeedLimit,
    ) -> Self {
        rcu::with_registers(|rcu| set_bits(rcu.apb2en(), APB2EN_SYSCFGEN));
        phy_reset.set_level(Level::Low);
        pins.configure();
        rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), AHB1EN_USBHSULPIEN | AHB1EN_USBHSEN));
        registers().dctl().modify(|control| control.set_sdis(true));
        phy_reset.set_level(Level::High);
        interrupt::enable(pac::Interrupt::USBHS, Priority::USB);
        let instance = OtgInstance {
            regs: registers(),
            state: STATE.as_state(),
            fifo_depth_words: FIFO_DEPTH_WORDS,
            phy_type: match limit {
                SpeedLimit::High => PhyType::ExternalHighSpeed,
                SpeedLimit::Full => PhyType::ExternalFullSpeed,
            },
            extra_rx_fifo_words: RECEIVE_FIFO_EXTRA_WORDS,
            calculate_trdt_fn: turnaround,
        };
        let mut config = Config::default();
        config.vbus_detection = false;
        config.xcvrdly = true;
        Self {
            inner: OtgDriver::new(out_buffer(), instance, config),
        }
    }
}

impl fmt::Debug for Driver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Driver").finish_non_exhaustive()
    }
}

impl<'d> DriverTrait<'d> for Driver {
    type Bus = Bus<'d>;
    type ControlPipe = ControlPipe<'d>;
    type EndpointIn = Endpoint<'d, In>;
    type EndpointOut = Endpoint<'d, Out>;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        self.inner
            .alloc_endpoint_out(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn alloc_endpoint_in(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointIn, EndpointAllocError> {
        self.inner
            .alloc_endpoint_in(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn start(self, control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        let (inner, control_pipe) = self.inner.start(control_max_packet_size);
        (
            Bus {
                inner,
                initialised: false,
            },
            control_pipe,
        )
    }
}

impl Bus<'_> {
    fn initialise_core(&mut self) {
        let registers = registers();
        while !registers.grstctl().read().ahbidl() {
            spin_loop();
        }
        self.inner.configure_as_device();
        self.inner.config_v1();
        registers
            .gccfg_v1()
            .modify(|configuration| configuration.set_novbussens(false));
        self.inner.core_soft_reset();
        self.inner.configure_as_device();
        registers.pcgcctl().write_value(Pcgcctl::default());
        registers.gusbcfg().modify(|configuration| {
            configuration.set_trdt(ULPI_TURNAROUND);
            configuration.set_tocal(TIMEOUT_CALIBRATION);
        });
        registers.gotgctl().modify(|control| {
            control.set_avaloen(false);
            control.set_bvaloen(true);
            control.set_bvaloval(true);
        });
    }
}

impl fmt::Debug for Bus<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Bus")
            .field("initialised", &self.initialised)
            .finish_non_exhaustive()
    }
}

impl BusTrait for Bus<'_> {
    async fn enable(&mut self) {
        self.inner.enable().await;
    }

    async fn disable(&mut self) {
        self.inner.disable().await;
    }

    async fn poll(&mut self) -> Event {
        if !self.initialised {
            self.initialise_core();
            self.initialised = true;
        }
        let event = self.inner.poll().await;
        if event == Event::Reset {
            let enumerated = registers().dsts().read().enumspd().to_bits();
            NEGOTIATED.store(encode(enumerated_speed(enumerated)), Ordering::Release);
        }
        event
    }

    fn endpoint_set_enabled(&mut self, ep_addr: EndpointAddress, enabled: bool) {
        self.inner.endpoint_set_enabled(ep_addr, enabled);
    }

    fn endpoint_set_stalled(&mut self, ep_addr: EndpointAddress, stalled: bool) {
        self.inner.endpoint_set_stalled(ep_addr, stalled);
    }

    fn endpoint_is_stalled(&mut self, ep_addr: EndpointAddress) -> bool {
        self.inner.endpoint_is_stalled(ep_addr)
    }

    async fn remote_wakeup(&mut self) -> Result<(), Unsupported> {
        self.inner.remote_wakeup().await
    }
}

impl UlpiPins {
    fn configure(self) {
        let _ = (
            ulpi(self.clock),
            ulpi(self.direction),
            ulpi(self.next),
            ulpi(self.stop),
            ulpi(self.data0),
            ulpi(self.data1),
            ulpi(self.data2),
            ulpi(self.data3),
            ulpi(self.data4),
            ulpi(self.data5),
            ulpi(self.data6),
            ulpi(self.data7),
        );
    }
}

fn ulpi<const PORT: char, const INDEX: u8>(
    pin: Pin<PORT, INDEX, Unconfigured>,
) -> Pin<PORT, INDEX, gpio::Alternate<ULPI_FUNCTION>> {
    pin.into_alternate::<ULPI_FUNCTION>(OutputType::PushPull, gpio::Speed::Maximum, Pull::None)
}

fn registers() -> Otg {
    // SAFETY: `Driver::new` consumed the USBHS peripherals, so the driver and its interrupt handler
    // are the only users of the register block from then on.
    unsafe { Otg::from_ptr(pac::HsGlobal::PTR.cast_mut().cast()) }
}

fn out_buffer() -> &'static mut [u8] {
    let buffer = &raw mut OUT_BUFFER;
    // SAFETY: `Driver::new` is the only caller and runs at most once, because it consumes the
    // USBHS peripherals that `pac::Peripherals::take` hands out once; so this is the only reference
    // to `OUT_BUFFER` that ever exists.
    unsafe { &mut *buffer }
}

const fn turnaround(_speed: Dspd) -> u8 {
    ULPI_TURNAROUND
}

const fn enumerated_speed(enumeration_speed: u8) -> Speed {
    match enumeration_speed {
        0b00 => Speed::High,
        0b10 => Speed::Low,
        _ => Speed::Full,
    }
}

const fn encode(speed: Speed) -> u8 {
    match speed {
        Speed::Low => 1,
        Speed::Full => 2,
        Speed::High => 3,
    }
}

const fn decode(encoded: u8) -> Option<Speed> {
    match encoded {
        1 => Some(Speed::Low),
        2 => Some(Speed::Full),
        3 => Some(Speed::High),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enumerated_speed_follows_the_device_status_encoding() {
        assert_eq!(enumerated_speed(0b00), Speed::High);
        assert_eq!(enumerated_speed(0b01), Speed::Full);
        assert_eq!(enumerated_speed(0b10), Speed::Low);
        assert_eq!(enumerated_speed(0b11), Speed::Full);
    }

    #[test]
    fn no_speed_is_reported_before_the_first_reset() {
        assert_eq!(negotiated_speed(), None);
        for speed in [Speed::Low, Speed::Full, Speed::High] {
            assert_eq!(decode(encode(speed)), Some(speed));
        }
    }
}
