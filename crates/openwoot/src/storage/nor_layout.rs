use openwoot_image::Region;

pub const SECTOR_SIZE: u32 = 4096;

pub(super) const MAXIMUM_SETTINGS_SECTORS: u32 = 128;

const MINIMUM_SETTINGS_SECTORS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NorLayout {
    pub capacity: u32,
    pub settings: Region,
    pub device_info: Option<Region>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsRegion(Region);

impl NorLayout {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let settings = self.settings;
        let settings_fit = settings.start.is_multiple_of(SECTOR_SIZE)
            && settings.end.is_multiple_of(SECTOR_SIZE)
            && settings.start < settings.end
            && (settings.end - settings.start) / SECTOR_SIZE >= MINIMUM_SETTINGS_SECTORS
            && (settings.end - settings.start) / SECTOR_SIZE <= MAXIMUM_SETTINGS_SECTORS
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
            Region {
                start: 0,
                end: (MAXIMUM_SETTINGS_SECTORS + 1) * SECTOR_SIZE,
            },
        ] {
            assert!(!layout(settings, None).is_valid(), "{settings:?}");
        }
    }
}
