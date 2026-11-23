use std::fmt;

use openwoot_protocol::Version;

use super::Error;
use crate::{channel, stock, transfer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Firmware {
    Openwoot(Version),
    OtherProtocol(String),
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
            Self::OtherProtocol(version) => write!(
                formatter,
                "openwoot firmware of another protocol, stock version {version}"
            ),
            Self::Stock(version) => write!(formatter, "stock firmware {version}"),
        }
    }
}

impl<R: transfer::Reports> Running<R> {
    pub async fn identify(mut reports: R) -> Result<Self, Error> {
        let firmware = match channel::firmware_version(&mut reports).await {
            Ok(Some(version)) => Firmware::Openwoot(version),
            Ok(None) => Firmware::Stock(stock_version(&mut reports).await?),
            Err(channel::Error::Reply(_)) => {
                Firmware::OtherProtocol(stock_version(&mut reports).await?)
            }
            Err(error) => return Err(error.into()),
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
        if let Firmware::Openwoot(_) = self.firmware {
            match channel::enter_update_mode(&mut self.reports).await {
                Ok(()) | Err(channel::Error::Transfer(transfer::Error::Disconnected)) => {
                    return Ok(());
                }
                Err(channel::Error::Reply(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        stock::enter_update_mode(&mut self.reports).await;
        Ok(())
    }
}

async fn stock_version(reports: &mut impl transfer::Reports) -> Result<String, Error> {
    stock::firmware_version(reports)
        .await?
        .ok_or(Error::NoVersionAnswer)
}
