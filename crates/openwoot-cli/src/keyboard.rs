mod firmware;
mod identity;

use std::time::Duration;

use async_hid::{Device, DeviceInfo};
pub use firmware::{Firmware, Running};
pub use identity::Identity;
use openwoot_protocol::Version;

use crate::{
    channel,
    hid::{self, Hid, Interface},
    stock, transfer,
};

pub const MODE_CHANGE_TIMEOUT: Duration = Duration::from_secs(20);

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
        "openwoot firmware {0} does not support the update-mode request; unplug the keyboard, \
         hold the recovery key combination while plugging it in, then run `openwoot flash` again"
    )]
    UpdateModeNotSupported(String),
    #[error("the keyboard in update mode did not appear within {} s", MODE_CHANGE_TIMEOUT.as_secs())]
    UpdateModeDidNotAppear,
    #[error("the firmware did not appear within {} s", MODE_CHANGE_TIMEOUT.as_secs())]
    FirmwareDidNotAppear,
}

pub async fn firmware_version(hid: &Hid) -> Result<(DeviceInfo, Version), Error> {
    let Some(device) = hid.find(identity::is_any_firmware).await? else {
        if hid.find(identity::is_any_update_mode).await?.is_some() {
            return Err(Error::InUpdateMode);
        }
        return Err(Error::NotFound);
    };
    let mut interface = Interface::open(&device).await?;
    let version = channel::firmware_version(&mut interface)
        .await?
        .ok_or(Error::NoChannelAnswer)?;
    Ok((device.to_device_info(), version))
}

pub async fn find_update_mode(hid: &Hid, identity: Identity) -> Result<Option<Device>, Error> {
    Ok(hid
        .find(|info: &DeviceInfo| identity.is_update_mode(info))
        .await?)
}

pub async fn open_firmware(hid: &Hid, identity: Identity) -> Result<Interface, Error> {
    let device = hid
        .find(|info: &DeviceInfo| identity.is_firmware(info))
        .await?
        .ok_or(Error::NotFound)?;
    Ok(Interface::open(&device).await?)
}

pub async fn wait_for_update_mode(hid: &Hid, identity: Identity) -> Result<Device, Error> {
    hid.wait_for(
        |info: &DeviceInfo| identity.is_update_mode(info),
        MODE_CHANGE_TIMEOUT,
    )
    .await?
    .ok_or(Error::UpdateModeDidNotAppear)
}

pub async fn wait_for_firmware(hid: &Hid, identity: Identity) -> Result<Device, Error> {
    hid.wait_for(
        |info: &DeviceInfo| identity.is_firmware(info),
        MODE_CHANGE_TIMEOUT,
    )
    .await?
    .ok_or(Error::FirmwareDidNotAppear)
}
