use core::convert::Infallible;

use embassy_usb_driver::{
    Direction, Driver as DriverTrait, EndpointAddress, EndpointAllocError, EndpointInfo,
    EndpointType,
};
use embedded_hal::digital::OutputPin;

use super::{
    bus::Bus,
    control_pipe::ControlPipe,
    endpoint::{DirectionMarker, Endpoint, In, Out},
    endpoint_slots::Slots,
    events::InterruptHandler,
    packet_memory::{self, Allocator, Buffer, CONTROL_PACKET_LIMIT},
    registers::{self, CTL_CLOSE, CTL_RSTIE, CTL_SETRST, CTL_SPSIE, CTL_STIE, CTL_WKUPIE},
};
use crate::{
    interrupt::{self, Binding, Priority, typelevel},
    pac,
    rcu::{self, Clocks, Hertz},
    register::{clear_bits, modify_bits, set_bits, write_bits},
};

const APB1EN_USBDEN: u32 = 1 << 23;
const APB2EN_AFEN: u32 = 1 << 0;
const CFG0_USBDPSC: u32 = 0b11 << 22 | 1 << 31;
const CFG0_USBDPSC_SHIFT: u32 = 22;
const ADDCTL_CK48MSEL: u32 = 1 << 0;
const PCF0_SWJ_CFG: u32 = 0b111 << 24;
const PCF0_SWJ_SERIAL_WIRE_ONLY: u32 = 0b010 << 24;
const STARTUP_MICROSECONDS: u32 = 1;

#[derive(Debug)]
pub struct Core {
    pub controller: pac::Usbd,
    pub packet_memory: pac::UsbdRam,
}

#[derive(Debug)]
pub struct Driver<PullUp> {
    pull_up: PullUp,
    slots: Slots,
    memory: Allocator,
}

impl<PullUp: OutputPin<Error = Infallible>> Driver<PullUp> {
    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "taking the peripherals hands the controller and its packet memory to the driver \
                  for good"
    )]
    pub fn new(
        core: Core,
        afio: &pac::Afio,
        mut pull_up: PullUp,
        _irqs: impl Binding<typelevel::USBD_HP_CAN0_TX, InterruptHandler>
        + Binding<typelevel::USBD_LP_CAN0_RX0, InterruptHandler>,
        clocks: &Clocks,
    ) -> Self {
        release_jtag_pins(afio);
        let Ok(()) = pull_up.set_low();
        rcu::with_registers(|rcu| {
            if let Some(prescaler) = prescaler(clocks.system) {
                modify_bits(rcu.cfg0(), |bits| (bits & !CFG0_USBDPSC) | prescaler);
            }
            clear_bits(rcu.addctl(), ADDCTL_CK48MSEL);
            set_bits(rcu.apb1en(), APB1EN_USBDEN);
        });
        let startup = clocks.system.0 / 1_000_000 * STARTUP_MICROSECONDS;
        let controller = &core.controller;
        write_bits(controller.ctl(), CTL_SETRST | CTL_CLOSE);
        cortex_m::asm::delay(startup);
        write_bits(controller.ctl(), CTL_SETRST);
        cortex_m::asm::delay(startup);
        write_bits(controller.baddr(), 0);
        interrupt::enable(pac::Interrupt::USBD_HP_CAN0_TX, Priority::USB);
        interrupt::enable(pac::Interrupt::USBD_LP_CAN0_RX0, Priority::USB);
        Self {
            pull_up,
            slots: Slots::default(),
            memory: Allocator::new(),
        }
    }

    fn alloc_endpoint<D: DirectionMarker>(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Endpoint<D>, EndpointAllocError> {
        let index = self
            .slots
            .find(
                ep_type,
                D::DIRECTION,
                ep_addr.map(|address| address.index()),
            )
            .ok_or(EndpointAllocError)?;
        let buffer = match D::DIRECTION {
            Direction::Out => {
                let receive = self
                    .memory
                    .receive(max_packet_size)
                    .ok_or(EndpointAllocError)?;
                describe_receive(index, receive.buffer, receive.count_field);
                receive.buffer
            }
            Direction::In => {
                let buffer = self
                    .memory
                    .transmit(max_packet_size)
                    .ok_or(EndpointAllocError)?;
                describe_transmit(index, buffer);
                buffer
            }
        };
        self.slots.claim(index, ep_type, D::DIRECTION);
        let info = EndpointInfo {
            addr: EndpointAddress::from_parts(index, D::DIRECTION),
            ep_type,
            max_packet_size,
            interval_ms,
        };
        Ok(Endpoint::new(info, buffer))
    }
}

impl<'d, PullUp: OutputPin<Error = Infallible> + 'd> DriverTrait<'d> for Driver<PullUp> {
    type Bus = Bus<PullUp>;
    type ControlPipe = ControlPipe;
    type EndpointIn = Endpoint<In>;
    type EndpointOut = Endpoint<Out>;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        self.alloc_endpoint(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn alloc_endpoint_in(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointIn, EndpointAllocError> {
        self.alloc_endpoint(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn start(self, control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        let max_packet_size = control_max_packet_size.min(CONTROL_PACKET_LIMIT);
        let receive = Buffer::control_receive(max_packet_size);
        describe_receive(0, receive.buffer, receive.count_field);
        let transmit = Buffer::control_transmit();
        describe_transmit(0, transmit);
        let control = |direction| EndpointInfo {
            addr: EndpointAddress::from_parts(0, direction),
            ep_type: EndpointType::Control,
            max_packet_size,
            interval_ms: 0,
        };
        let usbd = registers::usbd();
        write_bits(usbd.ctl(), 0);
        write_bits(usbd.intf(), 0);
        write_bits(usbd.ctl(), CTL_RSTIE | CTL_SPSIE | CTL_WKUPIE | CTL_STIE);
        (
            Bus::new(self.pull_up, self.slots.types()),
            ControlPipe::new(
                max_packet_size,
                Endpoint::new(control(Direction::In), transmit),
                Endpoint::new(control(Direction::Out), receive.buffer),
            ),
        )
    }
}

#[must_use]
pub const fn supports_system_clock(system: Hertz) -> bool {
    prescaler(system).is_some()
}

fn describe_receive(index: usize, buffer: Buffer, count_field: u16) {
    registers::store(packet_memory::receive_address_slot(index), buffer.address());
    registers::store(packet_memory::receive_count_slot(index), count_field);
}

fn describe_transmit(index: usize, buffer: Buffer) {
    registers::store(
        packet_memory::transmit_address_slot(index),
        buffer.address(),
    );
    registers::store(packet_memory::transmit_count_slot(index), 0);
}

fn release_jtag_pins(afio: &pac::Afio) {
    rcu::with_registers(|rcu| set_bits(rcu.apb2en(), APB2EN_AFEN));
    critical_section::with(|_| {
        modify_bits(afio.pcf0(), |bits| {
            (bits & !PCF0_SWJ_CFG) | PCF0_SWJ_SERIAL_WIRE_ONLY
        });
    });
}

const fn prescaler(pll: Hertz) -> Option<u32> {
    let field = match pll.0 {
        72_000_000 => 0b00,
        48_000_000 => 0b01,
        120_000_000 => 0b10,
        96_000_000 => 0b11,
        _ => return None,
    };
    Some(field << CFG0_USBDPSC_SHIFT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usb_clock_is_48_megahertz_from_the_supported_system_clocks() {
        assert_eq!(prescaler(Hertz::from_mhz(120)), Some(0x0080_0000));
        assert_eq!(prescaler(Hertz::from_mhz(96)), Some(0x00C0_0000));
        assert_eq!(prescaler(Hertz::from_mhz(72)), Some(0));
        assert_eq!(prescaler(Hertz::from_mhz(48)), Some(0x0040_0000));
        assert_eq!(prescaler(Hertz::from_mhz(108)), None);
    }

    #[test]
    fn only_system_clocks_with_a_48_megahertz_divider_are_supported() {
        for megahertz in [48, 72, 96, 120] {
            assert!(supports_system_clock(Hertz::from_mhz(megahertz)));
        }
        for megahertz in [8, 60, 108, 116] {
            assert!(!supports_system_clock(Hertz::from_mhz(megahertz)));
        }
    }
}
