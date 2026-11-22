use core::fmt;

use embassy_usb_driver::{
    Driver as DriverTrait, EndpointAddress, EndpointAllocError, EndpointType,
};
use embassy_usb_synopsys_otg::{
    Config, Driver as OtgDriver, Endpoint, In, OtgInstance, PhyType, otg_v1::vals::Dspd,
};

use super::{
    Bus, ControlPipe, InterruptHandler, OutEndpoint, UlpiPins, events::STATE, registers::registers,
    ulpi::ULPI_TURNAROUND,
};
use crate::{
    gpio::{Level, Output, Pin},
    interrupt::{self, Binding, Priority, typelevel},
    pac, rcu,
    register::set_bits,
};

const FIFO_DEPTH_WORDS: u16 = 1024;
const RECEIVE_FIFO_EXTRA_WORDS: u16 = 283;
const OUT_BUFFER_LEN: usize = 512;
const AHB1EN_USBHSEN: u32 = 1 << 29;
const AHB1EN_USBHSULPIEN: u32 = 1 << 30;
const APB2EN_SYSCFGEN: u32 = 1 << 14;

static mut OUT_BUFFER: [u8; OUT_BUFFER_LEN] = [0; OUT_BUFFER_LEN];

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

pub struct Driver {
    inner: OtgDriver<'static>,
}

impl Driver {
    #[must_use]
    pub fn new<const PORT: char, const INDEX: u8>(
        _core: Core,
        pins: UlpiPins,
        mut phy_reset: Pin<PORT, INDEX, Output>,
        _irqs: impl Binding<typelevel::USBHS, InterruptHandler>,
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
    type EndpointOut = OutEndpoint<'d>;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        let inner =
            self.inner
                .alloc_endpoint_out(ep_type, ep_addr, max_packet_size, interval_ms)?;
        Ok(OutEndpoint::new(inner))
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
        (Bus::new(inner), ControlPipe::new(control_pipe))
    }
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
