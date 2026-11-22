use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Kind {
    Input,
    Output,
    Feature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Usages {
    List(Vec<u32>),
    Range { minimum: u32, maximum: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) kind: Kind,
    pub(crate) report_id: u8,
    pub(crate) bit_offset: usize,
    pub(crate) size: usize,
    pub(crate) count: usize,
    pub(crate) usages: Usages,
    pub(crate) logical_minimum: i64,
    pub(crate) logical_maximum: i64,
    pub(crate) physical_minimum: i64,
    pub(crate) physical_maximum: i64,
    pub(crate) unit: u32,
    pub(crate) flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Collection {
    pub(crate) usage: u32,
    pub(crate) kind: u32,
    pub(crate) depth: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Parsed {
    pub(crate) fields: Vec<Field>,
    pub(crate) collections: Vec<Collection>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Globals {
    usage_page: u32,
    logical_minimum: i64,
    logical_maximum: i64,
    unsigned_logical_maximum: i64,
    physical_minimum: i64,
    physical_maximum: i64,
    unit: u32,
    size: usize,
    count: usize,
    report_id: u8,
}

#[derive(Debug, Default)]
struct Locals {
    usages: Vec<u32>,
    minimum: Option<u32>,
    maximum: Option<u32>,
}

impl Parsed {
    #[must_use]
    pub(crate) fn report_bits(&self, kind: Kind, report_id: u8) -> usize {
        self.fields
            .iter()
            .filter(|field| field.kind == kind && field.report_id == report_id)
            .map(|field| field.size * field.count)
            .sum()
    }

    #[must_use]
    pub(crate) fn reports(&self) -> BTreeMap<(Kind, u8), usize> {
        let mut reports = BTreeMap::new();
        for field in &self.fields {
            *reports.entry((field.kind, field.report_id)).or_insert(0) += field.size * field.count;
        }
        reports
    }

    #[must_use]
    pub(crate) fn applications(&self) -> Vec<u32> {
        self.collections
            .iter()
            .filter(|collection| collection.depth == 0)
            .map(|collection| collection.usage)
            .collect()
    }
}

impl Globals {
    fn field(&self, kind: Kind, usages: Usages, bit_offset: usize, flags: u32) -> Field {
        let logical_maximum = if self.logical_minimum >= 0 && self.logical_maximum < 0 {
            self.unsigned_logical_maximum
        } else {
            self.logical_maximum
        };
        Field {
            kind,
            report_id: self.report_id,
            bit_offset,
            size: self.size,
            count: self.count,
            usages,
            logical_minimum: self.logical_minimum,
            logical_maximum,
            physical_minimum: self.physical_minimum,
            physical_maximum: self.physical_maximum,
            unit: self.unit,
            flags,
        }
    }
}

pub(crate) fn parse(descriptor: &[u8]) -> Result<Parsed, String> {
    let mut parsed = Parsed::default();
    let mut globals = Globals::default();
    let mut locals = Locals::default();
    let mut offsets: BTreeMap<(Kind, u8), usize> = BTreeMap::new();
    let mut depth = 0_usize;
    let mut rest = descriptor;
    while let Some((&prefix, tail)) = rest.split_first() {
        if prefix == 0xFE {
            return Err("long items are not supported".to_owned());
        }
        let size = match prefix & 0x03 {
            3 => 4,
            other => usize::from(other),
        };
        let (data, tail) = tail
            .split_at_checked(size)
            .ok_or_else(|| format!("item 0x{prefix:02x} is cut off"))?;
        rest = tail;
        let unsigned = data
            .iter()
            .rev()
            .fold(0_u32, |value, &byte| value << 8 | u32::from(byte));
        let signed = match data {
            [] => 0,
            [byte] => i64::from(i8::from_le_bytes([*byte])),
            [low, high] => i64::from(i16::from_le_bytes([*low, *high])),
            _ => i64::from(unsigned.cast_signed()),
        };
        let local_usage = |usage: u32| {
            if size == 4 {
                usage
            } else {
                globals.usage_page << 16 | usage
            }
        };
        match prefix & 0xFC {
            0x80 | 0x90 | 0xB0 => {
                let kind = match prefix & 0xFC {
                    0x80 => Kind::Input,
                    0x90 => Kind::Output,
                    _ => Kind::Feature,
                };
                let usages = match (locals.minimum, locals.maximum) {
                    (Some(minimum), Some(maximum)) => Usages::Range { minimum, maximum },
                    _ => Usages::List(std::mem::take(&mut locals.usages)),
                };
                let offset = offsets.entry((kind, globals.report_id)).or_insert(0);
                parsed
                    .fields
                    .push(globals.field(kind, usages, *offset, unsigned));
                *offset += globals.size * globals.count;
                locals = Locals::default();
            }
            0xA0 => {
                parsed.collections.push(Collection {
                    usage: locals.usages.first().copied().unwrap_or_default(),
                    kind: unsigned,
                    depth,
                });
                depth += 1;
                locals = Locals::default();
            }
            0xC0 => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| "end collection without collection".to_owned())?;
            }
            0x04 => globals.usage_page = unsigned,
            0x14 => globals.logical_minimum = signed,
            0x24 => {
                globals.logical_maximum = signed;
                globals.unsigned_logical_maximum = i64::from(unsigned);
            }
            0x34 => globals.physical_minimum = signed,
            0x44 => globals.physical_maximum = signed,
            0x64 => globals.unit = unsigned,
            0x74 => globals.size = usize::try_from(unsigned).map_err(|error| error.to_string())?,
            0x84 => {
                globals.report_id = u8::try_from(unsigned).map_err(|error| error.to_string())?;
            }
            0x94 => {
                globals.count = usize::try_from(unsigned).map_err(|error| error.to_string())?;
            }
            0x08 => locals.usages.push(local_usage(unsigned)),
            0x18 => locals.minimum = Some(local_usage(unsigned)),
            0x28 => locals.maximum = Some(local_usage(unsigned)),
            other => return Err(format!("unknown item 0x{other:02x}")),
        }
    }
    if depth == 0 {
        Ok(parsed)
    } else {
        Err("collection is not closed".to_owned())
    }
}
