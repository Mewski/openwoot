use core::fmt;

use embassy_usb_driver::{Endpoint as EndpointTrait, EndpointError, EndpointInfo, EndpointOut};
use embassy_usb_synopsys_otg::{Endpoint, Out, otg_v1::regs::Doeptsiz};

use super::registers::registers;

pub(super) const CONTROL: usize = 0;

const SETUP_PACKETS: u8 = 3;

pub struct OutEndpoint<'d> {
    inner: Endpoint<'d, Out>,
}

impl<'d> OutEndpoint<'d> {
    pub(super) const fn new(inner: Endpoint<'d, Out>) -> Self {
        Self { inner }
    }
}

impl EndpointTrait for OutEndpoint<'_> {
    fn info(&self) -> &EndpointInfo {
        self.inner.info()
    }

    async fn wait_enabled(&mut self) {
        self.inner.wait_enabled().await;
    }
}

impl EndpointOut for OutEndpoint<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, EndpointError> {
        let len = self.inner.read(buf).await?;
        let info = self.inner.info();
        arm_out(info.addr.index(), info.max_packet_size);
        Ok(len)
    }
}

impl fmt::Debug for OutEndpoint<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutEndpoint")
            .field("info", self.inner.info())
            .finish()
    }
}

pub(super) fn arm_out(index: usize, max_packet_size: u16) {
    let registers = registers();
    critical_section::with(|_| {
        registers
            .doeptsiz(index)
            .write_value(out_transfer_size(index, max_packet_size));
        registers.doepctl(index).modify(|control| {
            control.set_cnak(true);
            control.set_epena(true);
        });
    });
}

fn out_transfer_size(index: usize, max_packet_size: u16) -> Doeptsiz {
    let mut size = Doeptsiz(0);
    size.set_xfrsiz(u32::from(max_packet_size));
    size.set_pktcnt(1);
    if index == CONTROL {
        size.set_rxdpid_stupcnt(SETUP_PACKETS);
    }
    size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_out_stage_is_armed_for_one_packet_as_the_vendor_library_does() {
        assert_eq!(out_transfer_size(CONTROL, 64).0, 0x6008_0040);
        assert_eq!(out_transfer_size(CONTROL, 8).0, 0x6008_0008);
        assert_eq!(out_transfer_size(3, 64).0, 0x0008_0040);
        assert_eq!(out_transfer_size(2, 512).0, 0x0008_0200);
    }
}
