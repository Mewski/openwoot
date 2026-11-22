use embedded_storage_async::nor_flash::{NorFlashError, ReadNorFlash};
use openwoot_image::Region;

use super::{DeviceInfo, DeviceInfoError, SECTOR_LEN, Unmarked};

pub trait DeviceInfoSource {
    fn read<F: ReadNorFlash>(
        &self,
        nor: &mut F,
    ) -> impl Future<Output = Result<DeviceInfo, DeviceInfoError>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NorSector {
    pub offset: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct McuPage {
    pub region: Region,
    pub read: fn(Region) -> Option<&'static [u8]>,
}

impl DeviceInfoSource for NorSector {
    async fn read<F: ReadNorFlash>(&self, nor: &mut F) -> Result<DeviceInfo, DeviceInfoError> {
        let mut sector = [0; SECTOR_LEN];
        nor.read(self.offset, &mut sector)
            .await
            .map_err(|error| DeviceInfoError::Flash(error.kind()))?;
        DeviceInfo::from_sector(&sector, Unmarked::ShortFormat)
    }
}

impl DeviceInfoSource for McuPage {
    fn read<F: ReadNorFlash>(
        &self,
        _nor: &mut F,
    ) -> impl Future<Output = Result<DeviceInfo, DeviceInfoError>> {
        let page = (self.read)(self.region).ok_or(DeviceInfoError::OutsideFlash);
        core::future::ready(
            page.and_then(|page| DeviceInfo::from_sector(page, Unmarked::RawRecord)),
        )
    }
}
