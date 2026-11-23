use core::{fmt, hint::spin_loop};

use embassy_usb_driver::{Bus as BusTrait, EndpointAddress, Event, Unsupported};
use embassy_usb_synopsys_otg::{Bus as OtgBus, otg_v1::regs::Pcgcctl};

use super::{events::ENDPOINT_COUNT, registers::registers, speed, ulpi::ULPI_TURNAROUND};

const TIMEOUT_CALIBRATION: u8 = 7;
const ALL_TRANSMIT_FIFOS: u8 = 0x10;

pub struct Bus<'d> {
    inner: OtgBus<'d>,
    initialized: bool,
}

impl<'d> Bus<'d> {
    pub(super) const fn new(inner: OtgBus<'d>) -> Self {
        Self {
            inner,
            initialized: false,
        }
    }

    fn initialize_core(&mut self) {
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
            .field("initialized", &self.initialized)
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
        if !self.initialized {
            self.initialize_core();
            self.initialized = true;
        }
        let event = self.inner.poll().await;
        if event == Event::Reset {
            speed::record(registers().dsts().read().enumspd().to_bits());
        }
        event
    }

    fn endpoint_set_enabled(&mut self, ep_addr: EndpointAddress, enabled: bool) {
        if exists(ep_addr) {
            self.inner.endpoint_set_enabled(ep_addr, enabled);
        }
    }

    fn endpoint_set_stalled(&mut self, ep_addr: EndpointAddress, stalled: bool) {
        if exists(ep_addr) {
            self.inner.endpoint_set_stalled(ep_addr, stalled);
        }
    }

    fn endpoint_is_stalled(&mut self, ep_addr: EndpointAddress) -> bool {
        exists(ep_addr) && self.inner.endpoint_is_stalled(ep_addr)
    }

    async fn remote_wakeup(&mut self) -> Result<(), Unsupported> {
        self.inner.remote_wakeup().await
    }
}

fn exists(ep_addr: EndpointAddress) -> bool {
    ep_addr.index() < ENDPOINT_COUNT
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

#[cfg(test)]
mod tests {
    use embassy_usb_driver::{Direction, Driver as _};
    use embassy_usb_synopsys_otg::{
        Config, Driver as OtgDriver, OtgInstance, PhyType, StateStorage,
        otg_v1::{Otg, vals::Dspd},
    };

    use super::*;

    const REGISTER_WORDS: usize = 0x400;
    const CONTROL_PACKET_SIZE: u16 = 64;

    fn bus() -> Bus<'static> {
        let words = Box::leak(Box::new([0_u32; REGISTER_WORDS]));
        // SAFETY: the leaked, zeroed words stand in for the register block of the controller;
        // they are aligned for word accesses, live for the rest of the test run and belong to
        // this bus alone.
        let regs = unsafe { Otg::from_ptr(words.as_mut_ptr().cast()) };
        let state = Box::leak(Box::new(StateStorage::<ENDPOINT_COUNT>::new()));
        let instance = OtgInstance {
            regs,
            state: state.as_state(),
            fifo_depth_words: 1024,
            phy_type: PhyType::ExternalHighSpeed,
            extra_rx_fifo_words: 283,
            calculate_trdt_fn: turnaround,
        };
        let out_buffer = Box::leak(Box::new([0_u8; 64]));
        let driver = OtgDriver::new(out_buffer, instance, Config::default());
        let (inner, _control_pipe) = driver.start(CONTROL_PACKET_SIZE);
        Bus::new(inner)
    }

    fn turnaround(_speed: Dspd) -> u8 {
        ULPI_TURNAROUND
    }

    #[test]
    fn an_endpoint_the_controller_lacks_never_reaches_the_drivers_assertions() {
        let mut bus = bus();
        for index in ENDPOINT_COUNT..16 {
            for direction in [Direction::Out, Direction::In] {
                let endpoint = EndpointAddress::from_parts(index, direction);
                bus.endpoint_set_stalled(endpoint, true);
                bus.endpoint_set_stalled(endpoint, false);
                bus.endpoint_set_enabled(endpoint, true);
                bus.endpoint_set_enabled(endpoint, false);
                assert!(!bus.endpoint_is_stalled(endpoint), "{endpoint:?}");
            }
        }
        for address in [0x06, 0x86, 0x0F, 0x8F, 0x7F, 0xFF] {
            let endpoint = EndpointAddress::from(address);
            bus.endpoint_set_stalled(endpoint, true);
            assert!(!bus.endpoint_is_stalled(endpoint), "{endpoint:?}");
        }
    }

    #[test]
    fn an_endpoint_of_the_controller_still_reaches_the_driver() {
        let mut bus = bus();
        for index in 0..ENDPOINT_COUNT {
            let endpoint = EndpointAddress::from_parts(index, Direction::Out);
            assert!(exists(endpoint));
            assert!(!bus.endpoint_is_stalled(endpoint));
            bus.endpoint_set_stalled(endpoint, true);
            assert!(bus.endpoint_is_stalled(endpoint));
            bus.endpoint_set_stalled(endpoint, false);
            assert!(!bus.endpoint_is_stalled(endpoint));
        }
    }
}
