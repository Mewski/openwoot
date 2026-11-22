use std::time::Duration;

use async_hid::{Device, DeviceInfo};
use openwoot_image::{UpdateMode, UsbIds};
use openwoot_protocol::Version;

use crate::{
    bootloader::PortError,
    channel,
    hid::{self, Hid, Interface},
    stock,
};

pub const VENDOR_ID: u16 = 0x31E3;
pub const MODE_CHANGE_TIMEOUT: Duration = Duration::from_secs(20);
const CHANNEL_USAGE_PAGE: u16 = 0xFF55;
const UPDATE_MODE_USAGE_PAGE: u16 = 0xFFFF;
const USB_MODE_COUNT: u16 = 3;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Port(#[from] PortError),
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
        "openwoot firmware {0} cannot be asked to enter update mode yet: the configuration \
         protocol has no such request; unplug the keyboard, hold the recovery key combination \
         while plugging it in, then run `openwoot flash` again"
    )]
    OpenwootCannotEnterUpdateMode(String),
    #[error("the keyboard in update mode did not appear within {} s", MODE_CHANGE_TIMEOUT.as_secs())]
    UpdateModeDidNotAppear,
    #[error("the firmware did not appear within {} s", MODE_CHANGE_TIMEOUT.as_secs())]
    FirmwareDidNotAppear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    usb: UsbIds,
    update_product_id: u16,
}

pub struct StockFirmware {
    interface: Interface,
    version: String,
}

impl Identity {
    #[must_use]
    pub const fn new(usb: UsbIds, update_mode: UpdateMode) -> Self {
        Self {
            usb,
            update_product_id: update_mode.product_id,
        }
    }

    #[must_use]
    pub fn is_update_mode(&self, info: &DeviceInfo) -> bool {
        info.vendor_id == self.usb.vendor_id
            && info.product_id == self.update_product_id
            && info.usage_page == UPDATE_MODE_USAGE_PAGE
    }

    #[must_use]
    pub fn is_firmware(&self, info: &DeviceInfo) -> bool {
        info.vendor_id == self.usb.vendor_id
            && info
                .product_id
                .checked_sub(self.usb.product_id_base)
                .is_some_and(|mode| mode < USB_MODE_COUNT)
            && info.usage_page == CHANNEL_USAGE_PAGE
    }
}

impl StockFirmware {
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub const fn info(&self) -> &DeviceInfo {
        self.interface.info()
    }

    pub async fn enter_update_mode(mut self) {
        stock::enter_update_mode(&mut self.interface).await;
    }
}

pub async fn firmware_version(hid: &Hid) -> Result<(DeviceInfo, Version), Error> {
    let Some(device) = hid.find(is_any_firmware).await? else {
        if hid.find(is_any_update_mode).await?.is_some() {
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

pub async fn stock_firmware(hid: &Hid, identity: Identity) -> Result<StockFirmware, Error> {
    let device = hid
        .find(|info: &DeviceInfo| identity.is_firmware(info))
        .await?
        .ok_or(Error::NotFound)?;
    let mut interface = Interface::open(&device).await?;
    if let Some(version) = stock::firmware_version(&mut interface).await? {
        return Ok(StockFirmware { interface, version });
    }
    match channel::firmware_version(&mut interface).await? {
        Some(version) => Err(Error::OpenwootCannotEnterUpdateMode(format_version(
            &version,
        ))),
        None => Err(Error::NoVersionAnswer),
    }
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

#[must_use]
pub fn format_version(version: &Version) -> String {
    let firmware = version.firmware;
    format!("{}.{}.{}", firmware.major, firmware.minor, firmware.patch)
}

fn is_any_firmware(info: &DeviceInfo) -> bool {
    info.vendor_id == VENDOR_ID && info.usage_page == CHANNEL_USAGE_PAGE
}

fn is_any_update_mode(info: &DeviceInfo) -> bool {
    info.vendor_id == VENDOR_ID && info.usage_page == UPDATE_MODE_USAGE_PAGE
}

#[cfg(test)]
mod tests {
    use async_hid::DeviceId;

    use super::*;

    fn info(product_id: u16, usage_page: u16) -> DeviceInfo {
        DeviceInfo {
            id: device_id(),
            name: "Wooting 80HE".to_owned(),
            manufacturer: None,
            product_id,
            vendor_id: VENDOR_ID,
            usage_id: 1,
            usage_page,
            serial_number: None,
        }
    }

    #[cfg(target_os = "macos")]
    fn device_id() -> DeviceId {
        DeviceId::RegistryEntryId(1)
    }

    #[cfg(target_os = "linux")]
    fn device_id() -> DeviceId {
        DeviceId::DevPath("/sys/class/hidraw/hidraw0".into())
    }

    #[cfg(target_os = "windows")]
    fn device_id() -> DeviceId {
        DeviceId::UncPath("\\\\?\\hid".into())
    }

    fn identity() -> Identity {
        Identity::new(
            UsbIds {
                vendor_id: VENDOR_ID,
                product_id_base: 0x1400,
            },
            UpdateMode {
                product_id: 0x140F,
                block_size: 256,
            },
        )
    }

    #[test]
    fn the_firmware_is_recognised_in_every_usb_mode() {
        let identity = identity();
        for product_id in 0x1400..0x1403 {
            assert!(identity.is_firmware(&info(product_id, CHANNEL_USAGE_PAGE)));
        }
        assert!(!identity.is_firmware(&info(0x1403, CHANNEL_USAGE_PAGE)));
        assert!(!identity.is_firmware(&info(0x1400, 0xFF53)));
    }

    #[test]
    fn update_mode_needs_its_product_id_and_usage_page() {
        let identity = identity();
        assert!(identity.is_update_mode(&info(0x140F, UPDATE_MODE_USAGE_PAGE)));
        assert!(!identity.is_update_mode(&info(0x140F, CHANNEL_USAGE_PAGE)));
        assert!(!identity.is_update_mode(&info(0x1400, UPDATE_MODE_USAGE_PAGE)));
    }
}
