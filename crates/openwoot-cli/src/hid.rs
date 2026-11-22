use std::time::{Duration, Instant};

use async_hid::{
    AsyncHidFeatureHandle, AsyncHidRead, AsyncHidWrite, Device, DeviceFeatureHandle, DeviceInfo,
    DeviceReader, DeviceWriter, HidBackend, HidError,
};
use async_io::Timer;
use futures_lite::{StreamExt, future};

use crate::bootloader::{Port, PortError, REQUEST_LEN, Reply, Request};

const NO_REPORT_ID: u8 = 0;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const DISCARD_BUFFER_LEN: usize = 4096;

#[derive(Debug, thiserror::Error)]
#[error("could not open {interface} ({hint})")]
pub struct OpenError {
    interface: String,
    hint: &'static str,
    source: PortError,
}

#[derive(Default)]
pub struct Hid {
    backend: HidBackend,
}

pub struct Interface {
    info: DeviceInfo,
    reader: DeviceReader,
    writer: DeviceWriter,
    feature: DeviceFeatureHandle,
}

pub struct UpdateMode(Interface);

pub trait Reports {
    fn write_output(&mut self, report: &[u8]) -> impl Future<Output = Result<(), PortError>>;

    fn write_feature(&mut self, report: &[u8]) -> impl Future<Output = Result<(), PortError>>;

    fn read_input(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> impl Future<Output = Result<Option<usize>, PortError>>;

    fn discard_input(&mut self) -> impl Future<Output = Result<(), PortError>>;
}

impl Hid {
    pub async fn find(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
    ) -> Result<Option<Device>, PortError> {
        let mut devices = self.backend.enumerate().await?;
        Ok(devices.find(|device| matches(device)).await)
    }

    pub async fn wait_for(
        &self,
        matches: impl Fn(&DeviceInfo) -> bool,
        timeout: Duration,
    ) -> Result<Option<Device>, PortError> {
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

impl Interface {
    pub async fn open(device: &Device) -> Result<Self, OpenError> {
        let handles = async {
            let (reader, writer) = device.open().await?;
            let feature = device.open_feature_handle().await?;
            Ok::<_, HidError>((reader, writer, feature))
        };
        let (reader, writer, feature) = handles.await.map_err(|error| OpenError {
            interface: describe(device),
            hint: access_hint(),
            source: error.into(),
        })?;
        Ok(Self {
            info: DeviceInfo::clone(device),
            reader,
            writer,
            feature,
        })
    }

    #[must_use]
    pub const fn info(&self) -> &DeviceInfo {
        &self.info
    }
}

impl Reports for Interface {
    async fn write_output(&mut self, report: &[u8]) -> Result<(), PortError> {
        Ok(self.writer.write_output_report(report).await?)
    }

    async fn write_feature(&mut self, report: &[u8]) -> Result<(), PortError> {
        Ok(self.feature.write_feature_report(report).await?)
    }

    async fn read_input(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> Result<Option<usize>, PortError> {
        let read = async { self.reader.read_input_report(report).await.map(Some) };
        let expiry = async {
            Timer::after(timeout).await;
            Ok(None)
        };
        Ok(future::or(read, expiry).await?)
    }

    async fn discard_input(&mut self) -> Result<(), PortError> {
        let mut report = vec![0; DISCARD_BUFFER_LEN];
        while let Some(read) = future::poll_once(self.reader.read_input_report(&mut report)).await {
            read?;
        }
        Ok(())
    }
}

impl UpdateMode {
    #[must_use]
    pub const fn new(interface: Interface) -> Self {
        Self(interface)
    }
}

impl Port for UpdateMode {
    async fn discard_replies(&mut self) -> Result<(), PortError> {
        self.0.discard_input().await
    }

    async fn send_request(&mut self, request: &Request) -> Result<(), PortError> {
        let mut report = [NO_REPORT_ID; 1 + REQUEST_LEN];
        if let Some(body) = report.get_mut(1..) {
            body.copy_from_slice(request);
        }
        self.0.write_feature(&report).await
    }

    async fn send_block(&mut self, block: &[u8]) -> Result<(), PortError> {
        let report: Vec<u8> = [NO_REPORT_ID].iter().chain(block).copied().collect();
        self.0.write_output(&report).await
    }

    async fn receive_reply(
        &mut self,
        reply: &mut Reply,
        timeout: Duration,
    ) -> Result<Option<usize>, PortError> {
        self.0.read_input(reply, timeout).await
    }

    async fn pause(&mut self, duration: Duration) {
        Timer::after(duration).await;
    }
}

impl From<HidError> for PortError {
    fn from(error: HidError) -> Self {
        match error {
            HidError::Disconnected | HidError::NotConnected => Self::Disconnected,
            error => Self::Transfer(error.to_string()),
        }
    }
}

#[must_use]
pub fn describe(info: &DeviceInfo) -> String {
    format!(
        "{:04x}:{:04x} \"{}\"",
        info.vendor_id, info.product_id, info.name
    )
}

const fn access_hint() -> &'static str {
    if cfg!(target_os = "linux") {
        "install crates/openwoot-cli/udev/70-openwoot.rules (docs/flashing.md) and replug the keyboard"
    } else {
        "close other programs that use the keyboard, such as Wootility"
    }
}
