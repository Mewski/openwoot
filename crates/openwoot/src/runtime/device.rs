use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_usb::{UsbDevice, driver::Driver};

use super::KeyReports;

pub struct Device<'d, D: Driver<'d>, M: RawMutex> {
    usb: UsbDevice<'d, D>,
    key_reports: &'d KeyReports<M>,
}

impl<'d, D: Driver<'d>, M: RawMutex> Device<'d, D, M> {
    #[must_use]
    pub fn new(usb: UsbDevice<'d, D>, key_reports: &'d KeyReports<M>) -> Self {
        Self { usb, key_reports }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            self.usb.run_until_suspend().await;
            let ended = select(self.usb.wait_resume(), self.key_reports.wake_up_requested()).await;
            if matches!(ended, Either::Second(())) {
                let _ = self.usb.remote_wakeup().await;
            }
            self.key_reports.forget_wake_up();
        }
    }
}

impl<'d, D: Driver<'d>, M: RawMutex> core::fmt::Debug for Device<'d, D, M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("Device").finish_non_exhaustive()
    }
}
