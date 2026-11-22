use std::time::Duration;

use async_hid::{
    AsyncHidFeatureHandle, AsyncHidRead, AsyncHidWrite, Device, DeviceFeatureHandle, DeviceInfo,
    DeviceReader, DeviceWriter, HidError,
};
use async_io::Timer;
use futures_lite::future;

use super::describe;
use crate::transfer::{self, Reports};

const DISCARD_BUFFER_LEN: usize = 4096;

#[derive(Debug, thiserror::Error)]
#[error("could not open {interface} ({hint})")]
pub struct OpenError {
    interface: String,
    hint: &'static str,
    source: transfer::Error,
}

pub struct Interface {
    info: DeviceInfo,
    reader: DeviceReader,
    writer: DeviceWriter,
    feature: DeviceFeatureHandle,
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
    async fn write_output(&mut self, report: &[u8]) -> Result<(), transfer::Error> {
        Ok(self.writer.write_output_report(report).await?)
    }

    async fn write_feature(&mut self, report: &[u8]) -> Result<(), transfer::Error> {
        Ok(self.feature.write_feature_report(report).await?)
    }

    async fn read_input(
        &mut self,
        report: &mut [u8],
        timeout: Duration,
    ) -> Result<Option<usize>, transfer::Error> {
        let read = async { self.reader.read_input_report(report).await.map(Some) };
        let expiry = async {
            Timer::after(timeout).await;
            Ok(None)
        };
        Ok(future::or(read, expiry).await?)
    }

    async fn discard_input(&mut self) -> Result<(), transfer::Error> {
        let mut report = vec![0; DISCARD_BUFFER_LEN];
        while let Some(read) = future::poll_once(self.reader.read_input_report(&mut report)).await {
            read?;
        }
        Ok(())
    }
}

impl From<HidError> for transfer::Error {
    fn from(error: HidError) -> Self {
        match error {
            HidError::Disconnected | HidError::NotConnected => Self::Disconnected,
            error => Self::Failed(error.to_string()),
        }
    }
}

const fn access_hint() -> &'static str {
    if cfg!(target_os = "linux") {
        "install crates/openwoot-cli/udev/70-openwoot.rules (docs/flashing.md) and replug the keyboard"
    } else {
        "close other programs that use the keyboard, such as Wootility"
    }
}
