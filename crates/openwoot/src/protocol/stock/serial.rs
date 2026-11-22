pub(crate) const LEGACY_LEN: usize = 11;
pub(crate) const ENCODED_CAPACITY: usize = 64;

const PRODUCT_ID_DIGITS: usize = 5;
const LEGACY_PRODUCT_ID_MAX: u32 = 0xFFFE;
const MINOR_REVISION_DIGITS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SerialNumber {
    supplier: u8,
    year: u8,
    week: u8,
    product: u8,
    revision: u8,
    product_id: u32,
    stage: u8,
    edition: Option<u8>,
    pcb_design: Option<u8>,
    minor_revision: Option<u8>,
}

#[derive(Debug)]
pub(crate) struct Encoded {
    bytes: [u8; ENCODED_CAPACITY],
    len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix('A')?;
        let (supplier, rest) = two_digits(rest)?;
        let rest = rest.strip_prefix('B')?;
        let (year, rest) = two_digits(rest)?;
        let (week, rest) = two_digits(rest)?;
        let rest = rest.strip_prefix('W')?;
        let (product, rest) = two_digits(rest)?;
        let (pcb_design, rest) = optional_tagged(rest, 'T')?;
        let digits = rest.len()
            - rest
                .trim_start_matches(|digit: char| digit.is_ascii_digit())
                .len();
        let (revision_digits, rest) = rest.split_at_checked(digits)?;
        let (revision, minor_revision) = match revision_digits.len() {
            1 | 2 => (revision_digits.parse().ok()?, None),
            3 | 4 => {
                let (revision, minor) =
                    revision_digits.split_at_checked(digits - MINOR_REVISION_DIGITS)?;
                (revision.parse().ok()?, Some(minor.parse().ok()?))
            }
            _ => return None,
        };
        let (edition, rest) = optional_tagged(rest, 'S')?;
        let mut chars = rest.chars();
        let stage = match chars.next()? {
            'H' => 0,
            'P' => 1,
            'T' => 2,
            'E' => 3,
            'X' => 4,
            _ => return None,
        };
        let product_id = chars.as_str();
        if product_id.len() < PRODUCT_ID_DIGITS
            || !product_id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        Some(Self {
            supplier,
            year,
            week,
            product,
            revision,
            product_id: product_id.parse().ok()?,
            stage,
            edition,
            pcb_design,
            minor_revision,
        })
    }

    pub(crate) fn to_legacy(self) -> [u8; LEGACY_LEN] {
        let [supplier_low, supplier_high] = u16::from(self.supplier).to_le_bytes();
        let [product_low, product_high] = u16::from(self.product).to_le_bytes();
        let [revision_low, revision_high] = u16::from(self.revision).to_le_bytes();
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

    pub(crate) fn encode(self) -> Encoded {
        let mut encoded = Encoded {
            bytes: [0; ENCODED_CAPACITY],
            len: 0,
        };
        encoded.field(Field::Supplier, self.supplier.into());
        encoded.field(Field::Year, self.year.into());
        encoded.field(Field::Week, self.week.into());
        encoded.field(Field::Product, self.product.into());
        encoded.field(Field::Revision, self.revision.into());
        encoded.field(Field::ProductId, self.product_id);
        encoded.field(Field::Stage, self.stage.into());
        for (field, value) in [
            (Field::Edition, self.edition),
            (Field::PcbDesign, self.pcb_design),
            (Field::MinorRevision, self.minor_revision),
        ] {
            if let Some(value) = value {
                encoded.field(field, value.into());
            }
        }
        encoded
    }
}

impl Encoded {
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }

    fn field(&mut self, field: Field, value: u32) {
        self.varint((field as u32) << 3);
        self.varint(value);
    }

    fn varint(&mut self, mut value: u32) {
        loop {
            let low = u8::try_from(value & 0x7F).unwrap_or_default();
            value >>= 7;
            let byte = if value == 0 { low } else { low | 0x80 };
            if let Some(slot) = self.bytes.get_mut(self.len) {
                *slot = byte;
                self.len += 1;
            }
            if value == 0 {
                break;
            }
        }
    }
}

fn two_digits(text: &str) -> Option<(u8, &str)> {
    let (digits, rest) = text.split_at_checked(2)?;
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, rest))
}

fn optional_tagged(text: &str, tag: char) -> Option<(Option<u8>, &str)> {
    match text.strip_prefix(tag) {
        Some(rest) => {
            let (value, rest) = two_digits(rest)?;
            Some((Some(value), rest))
        }
        None => Some((None, text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_SERIAL: &str = "A01B2401W01T00100S01H00300";

    #[test]
    fn the_serial_text_gives_the_legacy_block() {
        let serial = SerialNumber::parse(EXAMPLE_SERIAL).unwrap();
        assert_eq!(
            serial.to_legacy(),
            [
                0x01, 0x00, 0x18, 0x01, 0x01, 0x00, 0x01, 0x00, 0x2C, 0x01, 0x00
            ]
        );
    }

    #[test]
    fn the_message_has_every_required_field_and_the_present_optional_ones_in_tag_order() {
        let serial = SerialNumber::parse(EXAMPLE_SERIAL).unwrap();
        assert_eq!(
            serial.encode().as_bytes(),
            [
                0x08, 0x01, 0x10, 0x18, 0x18, 0x01, 0x20, 0x01, 0x28, 0x01, 0x30, 0xAC, 0x02, 0x38,
                0x00, 0x48, 0x01, 0x50, 0x00, 0x58, 0x00
            ]
        );
    }

    #[test]
    fn optional_parts_may_be_missing() {
        let serial = SerialNumber::parse("A01B2310W051P00042").unwrap();
        assert_eq!(serial.edition, None);
        assert_eq!(serial.pcb_design, None);
        assert_eq!(serial.minor_revision, None);
        assert_eq!(
            (serial.revision, serial.stage, serial.product_id),
            (1, 1, 42)
        );
        assert_eq!(
            serial.encode().as_bytes(),
            [
                0x08, 0x01, 0x10, 0x17, 0x18, 0x0A, 0x20, 0x05, 0x28, 0x01, 0x30, 0x2A, 0x38, 0x01
            ]
        );
    }

    #[test]
    fn text_in_another_format_is_no_serial_number() {
        for text in [
            "",
            "000000000000",
            "A01B2401W01T00100S01Q00300",
            "A01B2401W01T00100S01H0030",
            "A01B2401W01T0010000S01H00300",
            "A1B2401W01T00100S01H00300",
        ] {
            assert_eq!(SerialNumber::parse(text), None, "{text}");
        }
    }
}
