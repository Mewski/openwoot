mod firmware;
mod identity;

use std::time::{Duration, Instant};

use async_hid::{Device, DeviceInfo};
use async_io::Timer;
pub use firmware::{Firmware, Running};
pub use identity::Identity;
use openwoot_protocol::{PROTOCOL_VERSION, Version, diagnostics::Report};

use crate::{
    channel,
    hid::{self, Hid, Interface},
    stock,
    transfer::{self, Reports},
};

pub const MODE_CHANGE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug)]
pub enum Found {
    UpdateMode(Device),
    Firmware(Device),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Transfer(#[from] transfer::Error),
    #[error(transparent)]
    Open(#[from] hid::OpenError),
    #[error(transparent)]
    Stock(#[from] stock::Error),
    #[error(transparent)]
    Channel(#[from] channel::Error),
    #[error("no keyboard found; is it plugged in?")]
    NotFound,
    #[error(
        "{0} keyboards of the model the image is for are connected; connect only the one to flash"
    )]
    SeveralKeyboards(usize),
    #[error(
        "the keyboard is in update mode; flash an image, or unplug the keyboard and plug it in \
         again"
    )]
    InUpdateMode,
    #[error(
        "the keyboard does not answer on the openwoot configuration channel; it may run stock \
         firmware"
    )]
    NoChannelAnswer,
    #[error("the firmware did not answer the version query; nothing was changed")]
    NoVersionAnswer,
    #[error(
        "the firmware speaks configuration protocol {firmware}, this tool protocol {tool}; use the \
         `openwoot` built with the firmware"
    )]
    OtherProtocol { firmware: u16, tool: u16 },
    #[error("the keyboard in update mode did not appear within {} s", MODE_CHANGE_TIMEOUT.as_secs())]
    UpdateModeDidNotAppear,
    #[error(
        "the keyboard is still in update mode {} s after the restart command; run `openwoot \
         flash` again",
        MODE_CHANGE_TIMEOUT.as_secs()
    )]
    StillInUpdateMode,
    #[error(
        "the firmware did not appear within {} s; the first start after the stock firmware can \
         take longer, and if it never appears, start the keyboard in update mode with the key \
         combination (`docs/flashing.md`) and flash a known good image",
        MODE_CHANGE_TIMEOUT.as_secs()
    )]
    FirmwareDidNotAppear,
}

pub async fn firmware_version(hid: &Hid) -> Result<(DeviceInfo, Version), Error> {
    let (device, mut interface) = open_running(hid).await?;
    let version = channel::firmware_version(&mut interface)
        .await?
        .ok_or(Error::NoChannelAnswer)?;
    Ok((device, version))
}

pub async fn diagnostics(hid: &Hid) -> Result<(DeviceInfo, Report), Error> {
    let (device, mut interface) = open_running(hid).await?;
    let report = read_diagnostics(&mut interface).await?;
    Ok((device, report))
}

pub async fn read_diagnostics(reports: &mut impl Reports) -> Result<Report, Error> {
    let answer = channel::diagnostics(reports).await;
    match answer {
        Ok(Some(report)) => Ok(report),
        Ok(None) => Err(Error::NoChannelAnswer),
        Err(error @ channel::Error::Reply(_)) => match channel::firmware_version(reports).await {
            Ok(Some(version)) if version.protocol != PROTOCOL_VERSION => {
                Err(Error::OtherProtocol {
                    firmware: version.protocol,
                    tool: PROTOCOL_VERSION,
                })
            }
            _ => Err(error.into()),
        },
        Err(error) => Err(error.into()),
    }
}

pub async fn find(hid: &Hid, identity: Identity) -> Result<Found, Error> {
    let devices = hid
        .find_all(|info: &DeviceInfo| identity.is_update_mode(info) || identity.is_firmware(info))
        .await?;
    let count = devices.len();
    let mut devices = devices.into_iter();
    match (devices.next(), devices.next()) {
        (Some(device), None) if identity.is_update_mode(&device) => Ok(Found::UpdateMode(device)),
        (Some(device), None) => Ok(Found::Firmware(device)),
        (Some(_), Some(_)) => Err(Error::SeveralKeyboards(count)),
        (None, _) => Err(Error::NotFound),
    }
}

pub async fn open_update_mode(
    hid: &Hid,
    identity: Identity,
) -> Result<(DeviceInfo, Interface), Error> {
    let deadline = Instant::now() + MODE_CHANGE_TIMEOUT;
    loop {
        let failure = match hid
            .find(|info: &DeviceInfo| identity.is_update_mode(info))
            .await
        {
            Ok(Some(device)) => {
                let opened = Interface::open(&device).await;
                match opened {
                    Ok(interface) => return Ok((device.to_device_info(), interface)),
                    Err(error) if error.is_transient() => Error::Open(error),
                    Err(error) => return Err(error.into()),
                }
            }
            Ok(None) => Error::UpdateModeDidNotAppear,
            Err(error) => Error::Transfer(error),
        };
        if Instant::now() >= deadline {
            return Err(failure);
        }
        Timer::after(hid::POLL_INTERVAL).await;
    }
}

pub async fn wait_for_firmware(hid: &Hid, identity: Identity) -> Result<Device, Error> {
    if let Some(device) = hid
        .wait_for(
            |info: &DeviceInfo| identity.is_firmware(info),
            MODE_CHANGE_TIMEOUT,
        )
        .await?
    {
        return Ok(device);
    }
    if hid
        .find(|info: &DeviceInfo| identity.is_update_mode(info))
        .await?
        .is_some()
    {
        return Err(Error::StillInUpdateMode);
    }
    Err(Error::FirmwareDidNotAppear)
}

async fn open_running(hid: &Hid) -> Result<(DeviceInfo, Interface), Error> {
    let Some(device) = hid.find(identity::is_any_firmware).await? else {
        if hid.find(identity::is_any_update_mode).await?.is_some() {
            return Err(Error::InUpdateMode);
        }
        return Err(Error::NotFound);
    };
    let interface = Interface::open(&device).await?;
    Ok((device.to_device_info(), interface))
}
