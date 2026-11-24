mod interface;
mod update_mode;

use std::time::{Duration, Instant};

use async_hid::{Device, DeviceInfo, HidBackend};
use async_io::Timer;
use futures_lite::StreamExt;
pub use interface::{Interface, OpenError};
pub use update_mode::UpdateMode;

use crate::{escape, transfer};

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(500);

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

    pub async fn find_all(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
    ) -> Result<Vec<Device>, transfer::Error> {
        let found: Vec<Device> = self
            .backend
            .enumerate()
            .await?
            .filter(|device| matches(device))
            .collect()
            .await;
        let mut distinct: Vec<Device> = Vec::new();
        for device in found {
            if !distinct.iter().any(|known| known.id == device.id) {
                distinct.push(device);
            }
        }
        Ok(distinct)
    }

    pub async fn wait_for(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
        timeout: Duration,
    ) -> Result<Option<Device>, transfer::Error> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.find(&matches).await {
                Ok(Some(device)) => return Ok(Some(device)),
                Ok(None) | Err(_) if Instant::now() < deadline => {
                    Timer::after(POLL_INTERVAL).await;
                }
                last => return last,
            }
        }
    }
}

#[must_use]
pub fn describe(info: &DeviceInfo) -> String {
    format!(
        "{:04x}:{:04x} `{}`",
        info.vendor_id,
        info.product_id,
        escape::control_characters(&info.name)
    )
}
