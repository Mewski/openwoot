mod protobuf;

use core::fmt::{self, Write};

use embedded_storage_async::nor_flash::{NorFlashError, NorFlashErrorKind, ReadNorFlash};
use protobuf::{Fields, Wire, is_well_formed};

pub const DEVICE_INFO_LEN: usize = 4096;
pub const AUX_CAPACITY: usize = 58;
pub const SERIAL_TEXT_CAPACITY: usize = 48;

const AREA_LEN: usize = 0xFFC;
const MAGIC_SHORT: u16 = 0x1337;
const MAGIC_LONG: u16 = 0x1338;
const SHORT_PAYLOAD_MAX: usize = 254;
const LONG_PAYLOAD_MAX: usize = 0xFFA;
const BLANK_PROBE: usize = 30;
const ERASED: u8 = 0xFF;
const NOT_PROGRAMMED: u16 = 0xFFFF;
const LOCK_MODE_MASK: u8 = 0xFD;
const LOCKED: u8 = 1;
const RAW_AUX_LEN_AT: usize = 12;
const RAW_AUX_AT: usize = 13;
const RAW_AUX_CAPACITY: usize = 44;
const STAGE_LETTERS: [char; 4] = ['P', 'T', 'E', 'X'];
const STAGE_OTHER: char = 'H';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub serial: SerialNumber,
    pub aux: Option<AuxRecord>,
    pub physical_layout: Option<u8>,
    pub lock_mode: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SerialNumber {
    pub supplier: u16,
    pub year: u8,
    pub week: u8,
    pub product: u16,
    pub revision: u16,
    pub product_id: u32,
    pub stage: u8,
    pub edition: Option<u8>,
    pub pcb_design: Option<u8>,
    pub minor_revision: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxRecord {
    bytes: [u8; AUX_CAPACITY],
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerialText {
    bytes: [u8; SERIAL_TEXT_CAPACITY],
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unmarked {
    ShortFormat,
    RawRecord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NorSector {
    pub offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McuPage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DeviceInfoError {
    #[error("the device information is blank")]
    Blank,
    #[error("could not decode the device information")]
    Decode,
    #[error("could not read the device information: {0:?}")]
    Read(NorFlashErrorKind),
}

impl DeviceInfo {
    pub fn from_sector(sector: &[u8], unmarked: Unmarked) -> Result<Self, DeviceInfoError> {
        let area = sector.get(..AREA_LEN).ok_or(DeviceInfoError::Decode)?;
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

    #[must_use]
    pub const fn is_locked(&self) -> bool {
        matches!(self.lock_mode, Some(mode) if mode & LOCK_MODE_MASK == LOCKED)
    }

    fn from_short(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let (&len, rest) = area.split_first().ok_or(DeviceInfoError::Decode)?;
        let len = usize::from(len);
        if len == 0 || len > SHORT_PAYLOAD_MAX {
            return Err(DeviceInfoError::Decode);
        }
        Self::decode(rest.get(..len).ok_or(DeviceInfoError::Decode)?)
    }

    fn from_long(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let (len, rest) = area
            .split_first_chunk::<2>()
            .ok_or(DeviceInfoError::Decode)?;
        let len = usize::from(u16::from_le_bytes(*len));
        if len == 0 || len > LONG_PAYLOAD_MAX {
            return Err(DeviceInfoError::Decode);
        }
        Self::decode(rest.get(..len).ok_or(DeviceInfoError::Decode)?)
    }

    fn from_raw(area: &[u8]) -> Result<Self, DeviceInfoError> {
        let byte = |at: usize| area.get(at).copied().ok_or(DeviceInfoError::Decode);
        let half = |at: usize| Ok(u16::from_le_bytes([byte(at)?, byte(at + 1)?]));
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
                .ok_or(DeviceInfoError::Decode)?;
            Some(AuxRecord::from_message(bytes)?)
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
            match field.map_err(|_| DeviceInfoError::Decode)? {
                (1, Wire::Bytes(bytes)) => serial = Some(SerialNumber::decode(bytes)?),
                (2, Wire::Bytes(bytes)) => info.aux = Some(AuxRecord::from_message(bytes)?),
                (3, Wire::Varint(layout)) => info.physical_layout = Some(narrow(layout)?),
                (4, Wire::Varint(mode)) => info.lock_mode = Some(narrow(mode)?),
                (1..=4, _) => return Err(DeviceInfoError::Decode),
                _ => {}
            }
        }
        info.serial = serial.ok_or(DeviceInfoError::Decode)?;
        Ok(info)
    }
}

impl SerialNumber {
    #[must_use]
    pub const fn is_programmed(&self) -> bool {
        self.supplier != NOT_PROGRAMMED
    }

    #[must_use]
    pub fn text(&self) -> SerialText {
        let mut text = SerialText {
            bytes: [0; SERIAL_TEXT_CAPACITY],
            len: 0,
        };
        let _ = write!(text, "{self}");
        text
    }

    fn decode(message: &[u8]) -> Result<Self, DeviceInfoError> {
        let mut serial = Self::default();
        let mut required = 0u8;
        for field in Fields::new(message) {
            let (number, value) = match field.map_err(|_| DeviceInfoError::Decode)? {
                (number @ (1..=7 | 9..=11), Wire::Varint(value)) => (number, value),
                (1..=7 | 9..=11, _) => return Err(DeviceInfoError::Decode),
                _ => continue,
            };
            match number {
                1 => serial.supplier = narrow(value)?,
                2 => serial.year = narrow(value)?,
                3 => serial.week = narrow(value)?,
                4 => serial.product = narrow(value)?,
                5 => serial.revision = narrow(value)?,
                6 => serial.product_id = narrow(value)?,
                7 => serial.stage = narrow(value)?,
                9 => serial.edition = Some(narrow(value)?),
                10 => serial.pcb_design = Some(narrow(value)?),
                _ => serial.minor_revision = Some(narrow(value)?),
            }
            if number <= 7 {
                required |= 1 << (number - 1);
            }
        }
        if required == 0x7F {
            Ok(serial)
        } else {
            Err(DeviceInfoError::Decode)
        }
    }

    const fn stage_letter(&self) -> char {
        match self.stage {
            1 => STAGE_LETTERS[0],
            2 => STAGE_LETTERS[1],
            3 => STAGE_LETTERS[2],
            4 => STAGE_LETTERS[3],
            _ => STAGE_OTHER,
        }
    }
}

impl fmt::Display for SerialNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [supplier, ..] = self.supplier.to_le_bytes();
        let [product, ..] = self.product.to_le_bytes();
        let [revision, ..] = self.revision.to_le_bytes();
        write!(
            formatter,
            "A{supplier:02}B{:02}{:02}W{product:02}",
            self.year, self.week
        )?;
        if let Some(pcb_design) = self.pcb_design {
            write!(formatter, "T{pcb_design:02}")?;
        }
        write!(formatter, "{revision}")?;
        if let Some(minor_revision) = self.minor_revision {
            write!(formatter, "{minor_revision:02}")?;
        }
        if let Some(edition) = self.edition {
            write!(formatter, "S{edition:02}")?;
        }
        write!(formatter, "{}{:05}", self.stage_letter(), self.product_id)
    }
}

impl AuxRecord {
    fn from_message(message: &[u8]) -> Result<Self, DeviceInfoError> {
        let has_first = Fields::new(message).any(|field| matches!(field, Ok((1, Wire::Bytes(_)))));
        if !has_first || !is_well_formed(message) {
            return Err(DeviceInfoError::Decode);
        }
        let mut bytes = [0; AUX_CAPACITY];
        bytes
            .get_mut(..message.len())
            .ok_or(DeviceInfoError::Decode)?
            .copy_from_slice(message);
        Ok(Self {
            bytes,
            len: message.len(),
        })
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }
}

impl SerialText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.bytes.get(..self.len).unwrap_or_default()).unwrap_or_default()
    }
}

impl Write for SerialText {
    #[expect(
        clippy::renamed_function_params,
        reason = "the trait names the text `s`, which the naming rules reject"
    )]
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len + text.len();
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl NorSector {
    pub async fn read<F: ReadNorFlash>(
        &self,
        flash: &mut F,
    ) -> Result<DeviceInfo, DeviceInfoError> {
        let mut sector = [0; DEVICE_INFO_LEN];
        flash
            .read(self.offset, &mut sector)
            .await
            .map_err(|error| DeviceInfoError::Read(error.kind()))?;
        DeviceInfo::from_sector(&sector, Unmarked::ShortFormat)
    }
}

impl McuPage {
    pub fn read(self, page: &[u8]) -> Result<DeviceInfo, DeviceInfoError> {
        DeviceInfo::from_sector(page, Unmarked::RawRecord)
    }
}

fn narrow<T: TryFrom<u64>>(value: u64) -> Result<T, DeviceInfoError> {
    T::try_from(value).map_err(|_| DeviceInfoError::Decode)
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
        let mut sector = vec![ERASED; DEVICE_INFO_LEN];
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
    fn the_serial_text_follows_the_c_format() {
        let serial = SerialNumber::decode(&SERIAL_MESSAGE).unwrap();
        assert_eq!(serial.text().as_str(), "A01B2401W01T00100S01H00300");
        let short = SerialNumber {
            supplier: 0x0102,
            year: 23,
            week: 10,
            product: 5,
            revision: 1,
            product_id: 42,
            stage: 1,
            ..SerialNumber::default()
        };
        assert_eq!(short.text().as_str(), "A02B2310W051P00042");
        let large = SerialNumber {
            stage: 9,
            product_id: 4_000_000_000,
            ..short
        };
        assert_eq!(large.text().as_str(), "A02B2310W051H4000000000");
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
        assert!(from_short.is_locked());
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
        assert!(!info.is_locked());
        raw[12] = 200;
        let beyond =
            DeviceInfo::from_sector(&sector_with(&raw, None), Unmarked::RawRecord).unwrap();
        assert_eq!(beyond.aux, None);
    }

    #[test]
    fn broken_records_do_not_decode() {
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
                Err(DeviceInfoError::Decode),
                "{message:02x?}"
            );
        }
        let zero_length = sector_with(&[0x00, 0x0A], Some(MAGIC_SHORT));
        assert_eq!(
            DeviceInfo::from_sector(&zero_length, Unmarked::RawRecord),
            Err(DeviceInfoError::Decode)
        );
        let too_long = sector_with(&[0xFB, 0x0F], Some(MAGIC_LONG));
        assert_eq!(
            DeviceInfo::from_sector(&too_long, Unmarked::RawRecord),
            Err(DeviceInfoError::Decode)
        );
    }

    #[test]
    fn an_unprogrammed_serial_is_recognised() {
        let serial = SerialNumber {
            supplier: NOT_PROGRAMMED,
            ..SerialNumber::default()
        };
        assert!(!serial.is_programmed());
    }
}
