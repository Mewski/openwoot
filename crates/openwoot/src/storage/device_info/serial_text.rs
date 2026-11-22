use core::fmt::{self, Write};

use super::serial::SerialNumber;

const TEXT_CAPACITY: usize = 48;
const PRODUCT_ID_DIGITS: usize = 5;
const MINOR_REVISION_DIGITS: usize = 2;
const STAGE_LETTERS: [char; 4] = ['P', 'T', 'E', 'X'];
const STAGE_OTHER: char = 'H';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerialText {
    bytes: [u8; TEXT_CAPACITY],
    len: usize,
}

impl SerialNumber {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
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
            1 | 2 => (revision_digits.parse::<u8>().ok()?, None),
            3 | 4 => {
                let (revision, minor) =
                    revision_digits.split_at_checked(digits - MINOR_REVISION_DIGITS)?;
                (revision.parse::<u8>().ok()?, Some(minor.parse().ok()?))
            }
            _ => return None,
        };
        let (edition, rest) = optional_tagged(rest, 'S')?;
        let mut chars = rest.chars();
        let stage = match chars.next()? {
            STAGE_OTHER => 0,
            letter => {
                let index = STAGE_LETTERS.iter().position(|stage| *stage == letter)?;
                u8::try_from(index + 1).ok()?
            }
        };
        let product_id = chars.as_str();
        if product_id.len() < PRODUCT_ID_DIGITS
            || !product_id.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        Some(Self {
            supplier: supplier.into(),
            year,
            week,
            product: product.into(),
            revision: revision.into(),
            product_id: product_id.parse().ok()?,
            stage,
            edition,
            pcb_design,
            minor_revision,
        })
    }

    #[must_use]
    pub fn text(&self) -> SerialText {
        let mut text = SerialText {
            bytes: [0; TEXT_CAPACITY],
            len: 0,
        };
        let _ = write!(text, "{self}");
        text
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
    fn the_serial_text_formats_its_fields() {
        let serial = SerialNumber::parse(EXAMPLE_SERIAL).unwrap();
        assert_eq!(serial.text().as_str(), EXAMPLE_SERIAL);
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
    fn the_serial_text_parses_into_its_fields() {
        let serial = SerialNumber::parse(EXAMPLE_SERIAL).unwrap();
        assert_eq!(
            serial,
            SerialNumber {
                supplier: 1,
                year: 24,
                week: 1,
                product: 1,
                revision: 1,
                product_id: 300,
                stage: 0,
                edition: Some(1),
                pcb_design: Some(0),
                minor_revision: Some(0),
            }
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
