use core::fmt;

use embassy_usb_driver::{ControlPipe as ControlPipeTrait, EndpointError};
use embassy_usb_synopsys_otg::ControlPipe as OtgControlPipe;

use super::endpoint::{CONTROL, arm_out};

pub struct ControlPipe<'d> {
    inner: OtgControlPipe<'d>,
}

impl<'d> ControlPipe<'d> {
    pub(super) const fn new(inner: OtgControlPipe<'d>) -> Self {
        Self { inner }
    }

    fn control_packet_size(&self) -> u16 {
        u16::try_from(self.inner.max_packet_size()).unwrap_or(u16::MAX)
    }
}

impl ControlPipeTrait for ControlPipe<'_> {
    fn max_packet_size(&self) -> usize {
        self.inner.max_packet_size()
    }

    async fn setup(&mut self) -> [u8; 8] {
        self.inner.setup().await
    }

    async fn data_out(
        &mut self,
        buf: &mut [u8],
        first: bool,
        last: bool,
    ) -> Result<usize, EndpointError> {
        arm_out(CONTROL, self.control_packet_size());
        self.inner.data_out(buf, first, last).await
    }

    async fn data_in(&mut self, data: &[u8], first: bool, last: bool) -> Result<(), EndpointError> {
        if last {
            arm_out(CONTROL, self.control_packet_size());
        }
        self.inner.data_in(data, first, last).await
    }

    async fn accept(&mut self) {
        self.inner.accept().await;
    }

    async fn reject(&mut self) {
        self.inner.reject().await;
    }

    async fn accept_set_address(&mut self, addr: u8) {
        self.inner.accept_set_address(addr).await;
    }
}

impl fmt::Debug for ControlPipe<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlPipe")
            .finish_non_exhaustive()
    }
}
