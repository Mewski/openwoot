mod interface;
mod update_mode;

use std::time::{Duration, Instant};

use async_hid::{Device, DeviceInfo, HidBackend};
use async_io::Timer;
use futures_lite::StreamExt;
pub use interface::{Interface, OpenError};
pub use update_mode::UpdateMode;

use crate::transfer;

const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct Hid {
    backend: HidBackend,
}

impl Hid {
    pub async fn find(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
    ) -> Result<Option<Device>, transfer::Error> {
        let mut devices = self.backend.enumerate().await?;
        Ok(devices.find(|device| matches(device)).await)
    }

    pub async fn wait_for(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
        timeout: Duration,
    ) -> Result<Option<Device>, transfer::Error> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(device) = self.find(&matches).await? {
                return Ok(Some(device));
            }
            Timer::after(POLL_INTERVAL).await;
        }
        Ok(None)
    }
}

#[must_use]
pub fn describe(info: &DeviceInfo) -> String {
    format!(
        "{:04x}:{:04x} \"{}\"",
        info.vendor_id, info.product_id, info.name
    )
}
