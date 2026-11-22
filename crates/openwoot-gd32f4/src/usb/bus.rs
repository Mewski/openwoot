use core::{fmt, hint::spin_loop};

use embassy_usb_driver::{Bus as BusTrait, EndpointAddress, Event, Unsupported};
use embassy_usb_synopsys_otg::{Bus as OtgBus, otg_v1::regs::Pcgcctl};

use super::{registers::registers, speed, ulpi::ULPI_TURNAROUND};

const TIMEOUT_CALIBRATION: u8 = 7;
const ALL_TRANSMIT_FIFOS: u8 = 0x10;

pub struct Bus<'d> {
    inner: OtgBus<'d>,
    initialised: bool,
}

impl<'d> Bus<'d> {
    pub(super) const fn new(inner: OtgBus<'d>) -> Self {
        Self {
            inner,
            initialised: false,
        }
    }

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
        flush_fifos();
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
            speed::record(registers().dsts().read().enumspd().to_bits());
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

fn flush_fifos() {
    let registers = registers();
    registers
        .grstctl()
        .write(|control| control.set_rxfflsh(true));
    while registers.grstctl().read().rxfflsh() {
        spin_loop();
    }
    registers.grstctl().write(|control| {
        control.set_txfnum(ALL_TRANSMIT_FIFOS);
        control.set_txfflsh(true);
    });
    while registers.grstctl().read().txfflsh() {
        spin_loop();
    }
}
