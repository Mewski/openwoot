use std::fmt;

use openwoot_protocol::Version;

use super::Error;
use crate::{channel, stock, transfer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firmware {
    Openwoot(Version),
    Stock(String),
}

pub struct Running<R> {
    reports: R,
    firmware: Firmware,
}

impl fmt::Display for Firmware {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Openwoot(version) => write!(formatter, "openwoot firmware {}", version.firmware),
            Self::Stock(version) => write!(formatter, "stock firmware {version}"),
        }
    }
}

impl<R: transfer::Reports> Running<R> {
    pub async fn identify(mut reports: R) -> Result<Self, Error> {
        let firmware = if let Some(version) = channel::firmware_version(&mut reports).await? {
            Firmware::Openwoot(version)
        } else if let Some(version) = stock::firmware_version(&mut reports).await? {
            Firmware::Stock(version)
        } else {
            return Err(Error::NoVersionAnswer);
        };
        Ok(Self { reports, firmware })
    }

    #[must_use]
    pub const fn firmware(&self) -> &Firmware {
        &self.firmware
    }

    #[must_use]
    pub const fn reports(&self) -> &R {
        &self.reports
    }

    pub async fn enter_update_mode(mut self) -> Result<(), Error> {
        match self.firmware {
            Firmware::Stock(_) => {
                stock::enter_update_mode(&mut self.reports).await;
                Ok(())
            }
            Firmware::Openwoot(_) => match channel::enter_update_mode(&mut self.reports).await {
                Ok(()) | Err(channel::Error::Transfer(transfer::Error::Disconnected)) => Ok(()),
                Err(error) => Err(error.into()),
            },
        }
    }
}
