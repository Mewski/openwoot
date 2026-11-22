use openwoot_protocol::{FirmwareVersion, PROTOCOL_VERSION, Version};

#[must_use]
pub const fn version(major: &str, minor: &str, patch: &str) -> Version {
    Version {
        protocol: PROTOCOL_VERSION,
        firmware: FirmwareVersion {
            major: number(major),
            minor: number(minor),
            patch: number(patch),
        },
    }
}

const fn number(text: &str) -> u16 {
    let mut digits = text.as_bytes();
    let mut value: u16 = 0;
    while let [digit @ b'0'..=b'9', rest @ ..] = digits {
        value = value
            .saturating_mul(10)
            .saturating_add((*digit - b'0') as u16);
        digits = rest;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_version_becomes_the_firmware_version() {
        let reported = version("1", "20", "300");
        assert_eq!(reported.protocol, PROTOCOL_VERSION);
        assert_eq!(
            reported.firmware,
            FirmwareVersion {
                major: 1,
                minor: 20,
                patch: 300
            }
        );
        assert_eq!(number("70000"), u16::MAX);
    }
}
