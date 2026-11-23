use std::collections::BTreeMap;

const CONSTANT: u32 = 0x01;
const VARIABLE: u32 = 0x02;
const USAGE_ID: u32 = 0xFFFF;

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

    #[must_use]
    pub(crate) fn value(
        &self,
        kind: Kind,
        report_id: u8,
        usage: u32,
        payload: &[u8],
    ) -> Option<i64> {
        self.data_fields(kind, report_id)
            .find_map(|field| field.value(usage, payload))
    }

    #[must_use]
    pub(crate) fn pressed(&self, kind: Kind, report_id: u8, payload: &[u8]) -> Vec<u32> {
        self.data_fields(kind, report_id)
            .flat_map(|field| field.pressed(payload))
            .collect()
    }

    #[must_use]
    pub(crate) fn logical_range(
        &self,
        kind: Kind,
        report_id: u8,
        usage: u32,
    ) -> Option<(i64, i64)> {
        self.data_fields(kind, report_id)
            .find(|field| field.covers(usage))
            .map(|field| (field.logical_minimum, field.logical_maximum))
    }

    fn data_fields(&self, kind: Kind, report_id: u8) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(move |field| {
            field.kind == kind && field.report_id == report_id && field.flags & CONSTANT == 0
        })
    }
}

impl Field {
    fn is_array(&self) -> bool {
        self.flags & VARIABLE == 0
    }

    fn covers(&self, usage: u32) -> bool {
        match &self.usages {
            Usages::List(usages) => usages.contains(&usage),
            Usages::Range { minimum, maximum } => (*minimum..=*maximum).contains(&usage),
        }
    }

    fn slot(&self, usage: u32) -> Option<usize> {
        let slot = match &self.usages {
            Usages::List(usages) => usages.iter().position(|listed| *listed == usage)?,
            Usages::Range { minimum, maximum } => {
                if !(*minimum..=*maximum).contains(&usage) {
                    return None;
                }
                usize::try_from(usage - *minimum).ok()?
            }
        };
        (slot < self.count).then_some(slot)
    }

    fn usage_at(&self, slot: usize) -> Option<u32> {
        match &self.usages {
            Usages::List(usages) => usages.get(slot).copied(),
            Usages::Range { minimum, maximum } => {
                let usage = minimum.checked_add(u32::try_from(slot).ok()?)?;
                (usage <= *maximum).then_some(usage)
            }
        }
    }

    fn selected(&self, index: i64) -> Option<u32> {
        let Usages::Range { minimum, maximum } = &self.usages else {
            return None;
        };
        let offset = u32::try_from(index.checked_sub(self.logical_minimum)?).ok()?;
        let usage = minimum.checked_add(offset)?;
        (usage <= *maximum && usage & USAGE_ID != 0).then_some(usage)
    }

    fn read(&self, slot: usize, payload: &[u8]) -> i64 {
        let raw = (0..self.size).fold(0_i64, |value, bit| {
            let position = self.bit_offset + slot * self.size + bit;
            let set = payload
                .get(position / 8)
                .is_some_and(|byte| (byte >> (position % 8)) & 1 == 1);
            value | (i64::from(set) << bit)
        });
        let negative =
            self.logical_minimum < 0 && self.size > 0 && (raw >> (self.size - 1)) & 1 == 1;
        if negative {
            raw - (1_i64 << self.size)
        } else {
            raw
        }
    }

    fn value(&self, usage: u32, payload: &[u8]) -> Option<i64> {
        if self.is_array() {
            if !self.covers(usage) {
                return None;
            }
            let held =
                (0..self.count).any(|slot| self.selected(self.read(slot, payload)) == Some(usage));
            return Some(i64::from(held));
        }
        self.slot(usage).map(|slot| self.read(slot, payload))
    }

    fn pressed(&self, payload: &[u8]) -> Vec<u32> {
        (0..self.count)
            .filter_map(|slot| {
                let value = self.read(slot, payload);
                if self.is_array() {
                    self.selected(value)
                } else if value == 0 {
                    None
                } else {
                    self.usage_at(slot)
                }
            })
            .collect()
    }
}

impl Globals {
    fn field(&self, kind: Kind, usages: Usages, bit_offset: usize, flags: u32) -> Field {
        Field {
            kind,
            report_id: self.report_id,
            bit_offset,
            size: self.size,
            count: self.count,
            usages,
            logical_minimum: self.logical_minimum,
            logical_maximum: self.logical_maximum,
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
                if globals.logical_minimum > globals.logical_maximum {
                    return Err(format!(
                        "logical minimum {} lies above the logical maximum {}",
                        globals.logical_minimum, globals.logical_maximum
                    ));
                }
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
            0x24 => globals.logical_maximum = signed,
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
