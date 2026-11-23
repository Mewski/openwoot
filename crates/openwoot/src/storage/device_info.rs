mod aux_record;
mod location;
mod protobuf;
mod serial;
mod serial_text;

pub use aux_record::AuxRecord;
use embedded_storage_async::nor_flash::NorFlashErrorKind;
pub use location::{DeviceInfoSource, McuPage, NorSector};
use protobuf::{Fields, Malformed, Wire, narrow};
pub use serial::{LEGACY_SERIAL_LEN, SerialMessage, SerialNumber};
pub use serial_text::SerialText;

const SECTOR_LEN: usize = 4096;
const AREA_LEN: usize = 0xFFC;
const MAGIC_SHORT: u16 = 0x1337;
const MAGIC_LONG: u16 = 0x1338;
const SHORT_PAYLOAD_MAX: usize = 254;
const LONG_PAYLOAD_MAX: usize = 0xFFA;
const BLANK_PROBE: usize = 30;
const ERASED: u8 = 0xFF;
const RAW_AUX_LEN_AT: usize = 12;
const RAW_AUX_AT: usize = 13;
const RAW_AUX_CAPACITY: usize = 44;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: SerialNumber,
    pub aux: Option<AuxRecord>,
    pub physical_layout: Option<u8>,
    pub lock_mode: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unmarked {
    ShortFormat,
    RawRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeviceInfoError {
    #[error("the device information is blank")]
    Blank,
    #[error("the device information is malformed")]
    Malformed,
    #[error("could not read the device information: {0:?}")]
    Flash(NorFlashErrorKind),
    #[error("the device information lies outside the readable flash")]
    OutsideFlash,
}

impl DeviceInfo {
    fn from_sector(sector: &[u8], unmarked: Unmarked) -> Result<Self, DeviceInfoError> {
        let area = sector.get(..AREA_LEN).ok_or(DeviceInfoError::Malformed)?;
        let magic = sector
            .get(AREA_LEN..)
            .and_then(<[u8]>::first_chunk::<2>)
            .map(|magic| u16::from_le_bytes(*magic));
        match magic {
            Some(MAGIC_LONG) => Self::from_long(area),
            Some(MAGIC_SHORT) => Self::from_short(area),
            _ if area.iter().take(BLANK_PROBE).all(|byte| *byte == ERASED) => {
                Err(DeviceInfoError::Blank)
            }
            _ => match unmarked {
                Unmarked::ShortFormat => Self::from_short(area),
                Unmarked::RawRecord => Self::from_raw(area),
            },
        }
    }

    fn from_short(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let (&len, rest) = area.split_first().ok_or(DeviceInfoError::Malformed)?;
        let len = usize::from(len);
        if len == 0 || len > SHORT_PAYLOAD_MAX {
            return Err(DeviceInfoError::Malformed);
        }
        Self::decode(rest.get(..len).ok_or(DeviceInfoError::Malformed)?)
    }

    fn from_long(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let (len, rest) = area
            .split_first_chunk::<2>()
            .ok_or(DeviceInfoError::Malformed)?;
        let len = usize::from(u16::from_le_bytes(*len));
        if len == 0 || len > LONG_PAYLOAD_MAX {
            return Err(DeviceInfoError::Malformed);
        }
        Self::decode(rest.get(..len).ok_or(DeviceInfoError::Malformed)?)
    }

    fn from_raw(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let byte = |at: usize| area.get(at).copied().ok_or(DeviceInfoError::Malformed);
        let half = |at: usize| -> Result<u16, DeviceInfoError> {
            Ok(u16::from_le_bytes([byte(at)?, byte(at + 1)?]))
        };
        let serial = SerialNumber {
            supplier: half(0)?,
            year: byte(2)?,
            week: byte(3)?,
            product: half(4)?,
            revision: half(6)?,
            product_id: u32::from(half(8)?),
            stage: byte(10)?,
            ..SerialNumber::default()
        };
        let aux_len = usize::from(byte(RAW_AUX_LEN_AT)?);
        let aux = if (1..=RAW_AUX_CAPACITY).contains(&aux_len) {
            let bytes = area
                .get(RAW_AUX_AT..RAW_AUX_AT + aux_len)
                .ok_or(DeviceInfoError::Malformed)?;
            AuxRecord::from_message(bytes).ok()
        } else {
            None
        };
        Ok(Self {
            serial,
            aux,
            physical_layout: None,
            lock_mode: None,
        })
    }

    fn decode(message: &[u8]) -> Result<Self, DeviceInfoError> {
        let mut serial = None;
        let mut info = Self {
            serial: SerialNumber::default(),
            aux: None,
            physical_layout: None,
            lock_mode: None,
        };
        for field in Fields::new(message) {
            match field? {
                (1, Wire::Bytes(bytes)) => serial = Some(SerialNumber::decode(bytes)?),
                (2, Wire::Bytes(bytes)) => info.aux = AuxRecord::from_message(bytes).ok(),
                (3, Wire::Varint(layout)) => info.physical_layout = Some(narrow(layout)?),
                (4, Wire::Varint(mode)) => info.lock_mode = narrow(mode).ok(),
                (1 | 3, _) => return Err(DeviceInfoError::Malformed),
                _ => {}
            }
        }
        info.serial = serial.ok_or(DeviceInfoError::Malformed)?;
        Ok(info)
    }
}

impl From<Malformed> for DeviceInfoError {
    fn from(_: Malformed) -> Self {
        Self::Malformed
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{vec, vec::Vec};

    use super::*;

    const SERIAL_MESSAGE: [u8; 21] = [
        0x08, 0x01, 0x10, 0x18, 0x18, 0x01, 0x20, 0x01, 0x28, 0x01, 0x30, 0xAC, 0x02, 0x38, 0x00,
        0x48, 0x01, 0x50, 0x00, 0x58, 0x00,
    ];

    fn sector_with(prefix: &[u8], magic: Option<u16>) -> Vec<u8> {
        let mut sector = vec![ERASED; SECTOR_LEN];
        sector[..prefix.len()].copy_from_slice(prefix);
        if let Some(magic) = magic {
            sector[AREA_LEN..AREA_LEN + 2].copy_from_slice(&magic.to_le_bytes());
        }
        sector
    }

    fn record(serial: &[u8], rest: &[u8]) -> Vec<u8> {
        let mut message = vec![0x0A, u8::try_from(serial.len()).unwrap()];
        message.extend_from_slice(serial);
        message.extend_from_slice(rest);
        message
    }

    #[test]
    fn both_length_formats_decode_the_same_record() {
        let message = record(&SERIAL_MESSAGE, &[0x18, 0x01, 0x20, 0x03]);
        let mut short = vec![u8::try_from(message.len()).unwrap()];
        short.extend_from_slice(&message);
        let mut long = u16::try_from(message.len()).unwrap().to_le_bytes().to_vec();
        long.extend_from_slice(&message);
        let from_short =
            DeviceInfo::from_sector(&sector_with(&short, Some(MAGIC_SHORT)), Unmarked::RawRecord)
                .unwrap();
        let from_long =
            DeviceInfo::from_sector(&sector_with(&long, Some(MAGIC_LONG)), Unmarked::RawRecord)
                .unwrap();
        assert_eq!(from_short, from_long);
        assert_eq!(from_short.physical_layout, Some(1));
        assert_eq!(from_short.lock_mode, Some(3));
        assert!(from_short.serial.is_programmed());
    }

    #[test]
    fn a_sector_without_marker_is_blank_short_or_raw_by_strategy() {
        let blank = sector_with(&[], None);
        for unmarked in [Unmarked::ShortFormat, Unmarked::RawRecord] {
            assert_eq!(
                DeviceInfo::from_sector(&blank, unmarked),
                Err(DeviceInfoError::Blank)
            );
        }
        let message = record(&SERIAL_MESSAGE, &[]);
        let mut short = vec![u8::try_from(message.len()).unwrap()];
        short.extend_from_slice(&message);
        let unmarked = sector_with(&short, None);
        let info = DeviceInfo::from_sector(&unmarked, Unmarked::ShortFormat).unwrap();
        assert_eq!(info.serial.product_id, 300);
        assert_eq!(info.physical_layout, None);
    }

    #[test]
    fn the_older_raw_record_gives_serial_and_aux_record() {
        let aux = [0x0A, 0x02, 0x08, 0x05];
        let mut raw = vec![
            0x02, 0x00, 23, 10, 0x05, 0x00, 0x01, 0x00, 0x2A, 0x00, 0x01, 0x00,
        ];
        raw.push(u8::try_from(aux.len()).unwrap());
        raw.extend_from_slice(&aux);
        let info = DeviceInfo::from_sector(&sector_with(&raw, None), Unmarked::RawRecord).unwrap();
        assert_eq!(info.serial.text().as_str(), "A02B2310W051P00042");
        assert_eq!(info.aux.unwrap().as_bytes(), aux);
        assert_eq!((info.physical_layout, info.lock_mode), (None, None));
        raw[12] = 200;
        let beyond =
            DeviceInfo::from_sector(&sector_with(&raw, None), Unmarked::RawRecord).unwrap();
        assert_eq!(beyond.aux, None);
    }

    #[test]
    fn an_auxiliary_record_or_lock_mode_that_does_not_decode_is_left_out() {
        let mut too_long = vec![0x12, 59, 0x0A, 57];
        too_long.extend_from_slice(&[0x55; 57]);
        for unused in [
            too_long,
            vec![0x12, 0x02, 0xFF, 0xFF],
            vec![0x12, 0x02, 0x10, 0x05],
            vec![0x10, 0x05],
            vec![0x20, 0xAC, 0x02],
            vec![0x22, 0x00],
        ] {
            let mut rest = unused.clone();
            rest.extend_from_slice(&[0x18, 0x01]);
            let message = record(&SERIAL_MESSAGE, &rest);
            let mut short = vec![u8::try_from(message.len()).unwrap()];
            short.extend_from_slice(&message);
            let info = DeviceInfo::from_sector(
                &sector_with(&short, Some(MAGIC_SHORT)),
                Unmarked::ShortFormat,
            )
            .unwrap();
            assert_eq!(info.serial.product_id, 300, "{unused:02x?}");
            assert_eq!(info.physical_layout, Some(1), "{unused:02x?}");
            assert_eq!((info.aux, info.lock_mode), (None, None), "{unused:02x?}");
        }
        let mut raw = vec![
            0x02, 0x00, 23, 10, 0x05, 0x00, 0x01, 0x00, 0x2A, 0x00, 0x01, 0x00,
        ];
        raw.extend_from_slice(&[3, 0xFF, 0x00, 0x01]);
        let info = DeviceInfo::from_sector(&sector_with(&raw, None), Unmarked::RawRecord).unwrap();
        assert_eq!(info.serial.text().as_str(), "A02B2310W051P00042");
        assert_eq!(info.aux, None);
    }

    #[test]
    fn broken_records_are_malformed() {
        let missing_stage = &SERIAL_MESSAGE[..13];
        let too_wide = [
            0x08, 0x80, 0x80, 0x04, 0x10, 0x18, 0x18, 0x01, 0x20, 0x01, 0x28, 0x01, 0x30, 0x01,
            0x38, 0x00,
        ];
        for message in [
            record(missing_stage, &[]),
            record(&too_wide, &[]),
            record(&SERIAL_MESSAGE, &[0x18]),
            record(&SERIAL_MESSAGE, &[0x1A, 0x00]),
            vec![0x18, 0x01],
        ] {
            let mut short = vec![u8::try_from(message.len()).unwrap()];
            short.extend_from_slice(&message);
            assert_eq!(
                DeviceInfo::from_sector(
                    &sector_with(&short, Some(MAGIC_SHORT)),
                    Unmarked::RawRecord
                ),
                Err(DeviceInfoError::Malformed),
                "{message:02x?}"
            );
        }
        let zero_length = sector_with(&[0x00, 0x0A], Some(MAGIC_SHORT));
        assert_eq!(
            DeviceInfo::from_sector(&zero_length, Unmarked::RawRecord),
            Err(DeviceInfoError::Malformed)
        );
        let too_long = sector_with(&[0xFB, 0x0F], Some(MAGIC_LONG));
        assert_eq!(
            DeviceInfo::from_sector(&too_long, Unmarked::RawRecord),
            Err(DeviceInfoError::Malformed)
        );
    }
}
