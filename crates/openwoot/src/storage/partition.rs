use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};
use openwoot_image::Region;

pub const SECTOR_SIZE: u32 = 4096;

const MINIMUM_SETTINGS_SECTORS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NorLayout {
    pub capacity: u32,
    pub settings: Region,
    pub device_info: Option<Region>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsRegion(Region);

#[derive(Debug)]
pub struct Partition<F> {
    flash: F,
    start: u32,
    len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PartitionError<E> {
    #[error("could not access the flash: {0:?}")]
    Flash(E),
    #[error("the access lies outside the settings partition")]
    OutOfBounds,
}

impl NorLayout {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let settings = self.settings;
        let settings_fit = settings.start.is_multiple_of(SECTOR_SIZE)
            && settings.end.is_multiple_of(SECTOR_SIZE)
            && settings.start < settings.end
            && (settings.end - settings.start) / SECTOR_SIZE >= MINIMUM_SETTINGS_SECTORS
            && settings.end <= self.capacity;
        let device_info_apart = match self.device_info {
            Some(device_info) => {
                device_info.start.is_multiple_of(SECTOR_SIZE)
                    && device_info.end == device_info.start + SECTOR_SIZE
                    && device_info.end <= self.capacity
                    && (device_info.end <= settings.start || settings.end <= device_info.start)
            }
            None => true,
        };
        settings_fit && device_info_apart
    }

    #[must_use]
    pub const fn settings_region(&self) -> SettingsRegion {
        assert!(
            self.is_valid(),
            "the settings partition must be whole sectors that keep clear of the device information"
        );
        SettingsRegion(self.settings)
    }
}

impl SettingsRegion {
    #[must_use]
    pub const fn region(self) -> Region {
        self.0
    }
}

impl<F: NorFlash> Partition<F> {
    #[must_use]
    pub const fn new(flash: F, region: SettingsRegion) -> Self {
        Self {
            flash,
            start: region.0.start,
            len: region.0.end - region.0.start,
        }
    }

    #[must_use]
    pub const fn len(&self) -> u32 {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn release(self) -> F {
        self.flash
    }

    fn absolute<E>(&self, offset: u32, len: usize) -> Result<u32, PartitionError<E>> {
        let len = u32::try_from(len).map_err(|_| PartitionError::OutOfBounds)?;
        match offset.checked_add(len) {
            Some(end) if end <= self.len => Ok(self.start + offset),
            _ => Err(PartitionError::OutOfBounds),
        }
    }
}

impl<E: NorFlashError> NorFlashError for PartitionError<E> {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            Self::Flash(error) => error.kind(),
            Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
        }
    }
}

impl<F: NorFlash> ErrorType for Partition<F> {
    type Error = PartitionError<F::Error>;
}

impl<F: NorFlash> ReadNorFlash for Partition<F> {
    const READ_SIZE: usize = F::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let address = self.absolute(offset, bytes.len())?;
        self.flash
            .read(address, bytes)
            .await
            .map_err(PartitionError::Flash)
    }

    fn capacity(&self) -> usize {
        usize::try_from(self.len).unwrap_or_default()
    }
}

impl<F: NorFlash> NorFlash for Partition<F> {
    const ERASE_SIZE: usize = F::ERASE_SIZE;
    const WRITE_SIZE: usize = F::WRITE_SIZE;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let len = to.checked_sub(from).ok_or(PartitionError::OutOfBounds)?;
        let start = self.absolute(from, usize::try_from(len).unwrap_or(usize::MAX))?;
        self.flash
            .erase(start, start + len)
            .await
            .map_err(PartitionError::Flash)
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let address = self.absolute(offset, bytes.len())?;
        self.flash
            .write(address, bytes)
            .await
            .map_err(PartitionError::Flash)
    }
}

impl<F: MultiwriteNorFlash> MultiwriteNorFlash for Partition<F> {}

#[cfg(test)]
mod tests {
    use super::*;

    const CHIP: u32 = 0x80_0000;

    fn layout(settings: Region, device_info: Option<Region>) -> NorLayout {
        NorLayout {
            capacity: CHIP,
            settings,
            device_info,
        }
    }

    #[test]
    fn the_settings_must_keep_clear_of_the_device_information() {
        let last_sector = Region {
            start: 0x7F_F000,
            end: CHIP,
        };
        let settings = Region {
            start: 0,
            end: 0x8_0000,
        };
        assert!(layout(settings, Some(last_sector)).is_valid());
        assert!(layout(settings, None).is_valid());
        let over_device_info = Region {
            start: 0x7F_E000,
            end: CHIP,
        };
        assert!(!layout(over_device_info, Some(last_sector)).is_valid());
    }

    #[test]
    fn the_settings_are_whole_sectors_inside_the_chip() {
        for settings in [
            Region {
                start: 0x800,
                end: 0x8_0000,
            },
            Region {
                start: 0,
                end: 0x1000,
            },
            Region {
                start: 0x7F_0000,
                end: CHIP + 0x1000,
            },
            Region { start: 0, end: 0 },
        ] {
            assert!(!layout(settings, None).is_valid(), "{settings:?}");
        }
    }
}
