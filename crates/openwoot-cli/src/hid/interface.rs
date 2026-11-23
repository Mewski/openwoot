use std::{io, time::Duration};

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

impl OpenError {
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(
            self.source,
            transfer::Error::Disconnected | transfer::Error::NoAccess(_)
        )
    }
}

impl Interface {
    pub async fn open(device: &Device) -> Result<Self, OpenError> {
        let handles = async {
            let (reader, writer) = device.open().await?;
            let feature = device.open_feature_handle().await?;
            Ok::<_, HidError>((reader, writer, feature))
        };
        let (reader, writer, feature) = handles.await.map_err(|error| {
            let source = transfer::Error::from(error);
            OpenError {
                interface: describe(device),
                hint: hint(&source),
                source,
            }
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
            HidError::Other(source)
                if source
                    .downcast_ref::<io::Error>()
                    .is_some_and(|failure| failure.kind() == io::ErrorKind::PermissionDenied) =>
            {
                Self::NoAccess(source.to_string())
            }
            error => Self::Failed(error.to_string()),
        }
    }
}

fn hint(error: &transfer::Error) -> &'static str {
    match error {
        transfer::Error::NoAccess(_) if cfg!(target_os = "linux") => {
            "install `crates/openwoot-cli/udev/70-openwoot.rules` (`docs/flashing.md`) and replug \
             the keyboard"
        }
        transfer::Error::Disconnected => "replug the keyboard and run the command again",
        transfer::Error::NoAccess(_) | transfer::Error::Failed(_) => {
            "close other programs that use the keyboard, such as Wootility"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UDEV_RULE: &str = "70-openwoot.rules";

    fn open_error(source: transfer::Error) -> OpenError {
        OpenError {
            interface: "31e3:140f `Wooting Restore (80HE)`".to_owned(),
            hint: hint(&source),
            source,
        }
    }

    #[test]
    fn a_denied_open_keeps_its_kind_and_is_retried() {
        let denied = transfer::Error::from(HidError::from(io::Error::from(
            io::ErrorKind::PermissionDenied,
        )));
        assert!(matches!(denied, transfer::Error::NoAccess(_)));
        assert_eq!(hint(&denied).contains(UDEV_RULE), cfg!(target_os = "linux"));
        assert!(open_error(denied).is_transient());
    }

    #[test]
    fn a_device_that_left_usb_is_retried() {
        let gone = transfer::Error::from(HidError::NotConnected);
        assert_eq!(gone, transfer::Error::Disconnected);
        assert!(open_error(gone).is_transient());
    }

    #[test]
    fn any_other_failure_ends_at_once_without_the_udev_hint() {
        let busy = transfer::Error::from(HidError::message("device busy"));
        assert_eq!(busy, transfer::Error::Failed("device busy".to_owned()));
        assert!(!hint(&busy).contains(UDEV_RULE));
        assert!(!open_error(busy).is_transient());
    }
}
