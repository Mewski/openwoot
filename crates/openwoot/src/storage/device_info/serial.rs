use super::protobuf::{Fields, Malformed, Wire, Writer, narrow};

pub const LEGACY_SERIAL_LEN: usize = 11;
pub const SERIAL_MESSAGE_CAPACITY: usize = 64;

const NOT_PROGRAMMED: u16 = 0xFFFF;
const REQUIRED_FIELDS: u8 = 0x7F;
const LEGACY_PRODUCT_ID_MAX: u32 = 0xFFFE;

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
pub struct SerialMessage {
    bytes: [u8; SERIAL_MESSAGE_CAPACITY],
    len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Field {
    Supplier = 1,
    Year = 2,
    Week = 3,
    Product = 4,
    Revision = 5,
    ProductId = 6,
    Stage = 7,
    Edition = 9,
    PcbDesign = 10,
    MinorRevision = 11,
}

impl SerialNumber {
    #[must_use]
    pub const fn is_programmed(&self) -> bool {
        self.supplier != NOT_PROGRAMMED
    }

    #[must_use]
    pub fn to_legacy(self) -> [u8; LEGACY_SERIAL_LEN] {
        let [supplier_low, supplier_high] = self.supplier.to_le_bytes();
        let [product_low, product_high] = self.product.to_le_bytes();
        let [revision_low, revision_high] = self.revision.to_le_bytes();
        let clamped = u16::try_from(self.product_id.min(LEGACY_PRODUCT_ID_MAX)).unwrap_or(u16::MAX);
        let [id_low, id_high] = clamped.to_le_bytes();
        [
            supplier_low,
            supplier_high,
            self.year,
            self.week,
            product_low,
            product_high,
            revision_low,
            revision_high,
            id_low,
            id_high,
            self.stage,
        ]
    }

    #[must_use]
    pub fn encode(self) -> SerialMessage {
        let mut bytes = [0; SERIAL_MESSAGE_CAPACITY];
        let mut writer = Writer::new(&mut bytes);
        for (field, value) in [
            (Field::Supplier, self.supplier.into()),
            (Field::Year, self.year.into()),
            (Field::Week, self.week.into()),
            (Field::Product, self.product.into()),
            (Field::Revision, self.revision.into()),
            (Field::ProductId, self.product_id),
            (Field::Stage, self.stage.into()),
        ] {
            writer.varint_field(field as u32, value);
        }
        for (field, value) in [
            (Field::Edition, self.edition),
            (Field::PcbDesign, self.pcb_design),
            (Field::MinorRevision, self.minor_revision),
        ] {
            if let Some(value) = value {
                writer.varint_field(field as u32, value.into());
            }
        }
        let len = writer.len();
        SerialMessage { bytes, len }
    }

    pub(super) fn decode(message: &[u8]) -> Result<Self, Malformed> {
        let mut serial = Self::default();
        let mut required = 0u8;
        for field in Fields::new(message) {
            let (number, value) = match field? {
                (number @ (1..=7 | 9..=11), Wire::Varint(value)) => (number, value),
                (1..=7 | 9..=11, _) => return Err(Malformed),
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
        if required == REQUIRED_FIELDS {
            Ok(serial)
        } else {
            Err(Malformed)
        }
    }
}

impl SerialMessage {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_MESSAGE: [u8; 21] = [
        0x08, 0x01, 0x10, 0x18, 0x18, 0x01, 0x20, 0x01, 0x28, 0x01, 0x30, 0xAC, 0x02, 0x38, 0x00,
        0x48, 0x01, 0x50, 0x00, 0x58, 0x00,
    ];

    #[test]
    fn the_serial_gives_the_legacy_block() {
        let serial = SerialNumber::decode(&EXAMPLE_MESSAGE).unwrap();
        assert_eq!(
            serial.to_legacy(),
            [
                0x01, 0x00, 0x18, 0x01, 0x01, 0x00, 0x01, 0x00, 0x2C, 0x01, 0x00
            ]
        );
    }

    #[test]
    fn the_message_has_every_required_field_and_the_present_optional_ones_in_tag_order() {
        let serial = SerialNumber::decode(&EXAMPLE_MESSAGE).unwrap();
        assert_eq!(serial.encode().as_bytes(), EXAMPLE_MESSAGE);
        let without_options = SerialNumber {
            supplier: 1,
            year: 23,
            week: 10,
            product: 5,
            revision: 1,
            product_id: 42,
            stage: 1,
            ..SerialNumber::default()
        };
        assert_eq!(
            without_options.encode().as_bytes(),
            [
                0x08, 0x01, 0x10, 0x17, 0x18, 0x0A, 0x20, 0x05, 0x28, 0x01, 0x30, 0x2A, 0x38, 0x01
            ]
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
