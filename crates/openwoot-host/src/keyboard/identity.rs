use async_hid::DeviceInfo;
use openwoot_image::{UpdateMode, UsbIds};

const VENDOR_ID: u16 = 0x31E3;
const CHANNEL_USAGE_PAGE: u16 = 0xFF55;
const UPDATE_MODE_USAGE_PAGE: u16 = 0xFFFF;
const USB_MODE_COUNT: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    usb: UsbIds,
    update_product_id: u16,
}

impl Identity {
    #[must_use]
    pub const fn new(usb: UsbIds, update_mode: UpdateMode) -> Self {
        Self {
            usb,
            update_product_id: update_mode.product_id,
        }
    }

    pub(super) fn is_update_mode(self, info: &DeviceInfo) -> bool {
        info.vendor_id == self.usb.vendor_id
            && info.product_id == self.update_product_id
            && info.usage_page == UPDATE_MODE_USAGE_PAGE
    }

    pub(super) fn is_firmware(self, info: &DeviceInfo) -> bool {
        info.vendor_id == self.usb.vendor_id
            && info
                .product_id
                .checked_sub(self.usb.product_id_base)
                .is_some_and(|mode| mode < USB_MODE_COUNT)
            && info.usage_page == CHANNEL_USAGE_PAGE
    }
}

pub(super) fn is_any_firmware(info: &DeviceInfo) -> bool {
    info.vendor_id == VENDOR_ID && info.usage_page == CHANNEL_USAGE_PAGE
}

pub(super) fn is_any_update_mode(info: &DeviceInfo) -> bool {
    info.vendor_id == VENDOR_ID && info.usage_page == UPDATE_MODE_USAGE_PAGE
}

#[cfg(test)]
mod tests {
    use async_hid::DeviceId;
    use openwoot_image::UpdateProtocol;

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
                protocol: UpdateProtocol::WootingFactoryBootloader,
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

    #[test]
    fn another_vendor_with_the_same_product_ids_is_not_the_keyboard() {
        let identity = identity();
        let foreign = |product_id, usage_page| DeviceInfo {
            vendor_id: 0x1234,
            ..info(product_id, usage_page)
        };
        assert!(!identity.is_firmware(&foreign(0x1400, CHANNEL_USAGE_PAGE)));
        assert!(!identity.is_update_mode(&foreign(0x140F, UPDATE_MODE_USAGE_PAGE)));
        assert!(!is_any_firmware(&foreign(0x1400, CHANNEL_USAGE_PAGE)));
        assert!(!is_any_update_mode(&foreign(
            0x140F,
            UPDATE_MODE_USAGE_PAGE
        )));
    }

    #[test]
    fn any_keyboard_is_recognised_by_vendor_id_and_usage_page_only() {
        assert!(is_any_firmware(&info(0x1300, CHANNEL_USAGE_PAGE)));
        assert!(!is_any_firmware(&info(0x1400, 0xFF54)));
        assert!(!is_any_firmware(&info(0x1400, UPDATE_MODE_USAGE_PAGE)));
        assert!(is_any_update_mode(&info(0x130F, UPDATE_MODE_USAGE_PAGE)));
        assert!(!is_any_update_mode(&info(0x140F, CHANNEL_USAGE_PAGE)));
    }
}
