use super::{endpoint_register::Reading, packet_memory::ENDPOINT_COUNT};
use crate::{
    pac::{self, usbd::Ep0cs},
    register::{read_bits, write_bits},
};

const ENDPOINT_REGISTER_STRIDE: usize = 4;
const PACKET_MEMORY_HALFWORDS: usize = 256;

pub(super) const CTL_SETRST: u16 = 1 << 0;
pub(super) const CTL_CLOSE: u16 = 1 << 1;
pub(super) const CTL_LOWM: u16 = 1 << 2;
pub(super) const CTL_SETSPS: u16 = 1 << 3;
pub(super) const CTL_RSREQ: u16 = 1 << 4;
pub(super) const CTL_RSTIE: u16 = 1 << 10;
pub(super) const CTL_SPSIE: u16 = 1 << 11;
pub(super) const CTL_WKUPIE: u16 = 1 << 12;
pub(super) const CTL_STIE: u16 = 1 << 15;
pub(super) const INTF_EPNUM: u16 = 0b1111;
pub(super) const INTF_RSTIF: u16 = 1 << 10;
pub(super) const INTF_SPSIF: u16 = 1 << 11;
pub(super) const INTF_WKUPIF: u16 = 1 << 12;
pub(super) const INTF_STIF: u16 = 1 << 15;
pub(super) const ADDR_USBEN: u16 = 1 << 7;

pub(super) fn usbd() -> &'static pac::usbd::RegisterBlock {
    // SAFETY: `Driver::new` consumed the `Usbd` peripheral; afterwards only this driver's parts use
    // its registers. Read-modify-writes of `CTL` outside the USB interrupt run in critical
    // sections, and endpoint registers are written with values that leave the bits the hardware
    // changes untouched.
    unsafe { &*pac::Usbd::ptr() }
}

pub(super) fn endpoint(index: usize) -> Reading {
    Reading(read_bits(endpoint_cell(index)))
}

pub(super) fn write_endpoint(index: usize, value: u16) {
    write_bits(endpoint_cell(index), value);
}

pub(super) fn load(halfword: usize) -> u16 {
    read_bits(packet_memory().data(halfword % PACKET_MEMORY_HALFWORDS))
}

pub(super) fn store(halfword: usize, value: u16) {
    write_bits(
        packet_memory().data(halfword % PACKET_MEMORY_HALFWORDS),
        value,
    );
}

fn endpoint_cell(index: usize) -> &'static Ep0cs {
    let offset = (index % ENDPOINT_COUNT) * ENDPOINT_REGISTER_STRIDE;
    // SAFETY: the eight endpoint registers share one 16-bit layout and lie 4 bytes apart from
    // `EP0CS`; the offset stays below the eighth. Ownership is that of `usbd`.
    unsafe { &*pac::Usbd::ptr().cast::<Ep0cs>().byte_add(offset) }
}

fn packet_memory() -> &'static pac::usbd_ram::RegisterBlock {
    // SAFETY: `Driver::new` consumed the `UsbdRam` peripheral; afterwards only this driver uses
    // the packet memory, each buffer through the endpoint that owns it.
    unsafe { &*pac::UsbdRam::ptr() }
}
