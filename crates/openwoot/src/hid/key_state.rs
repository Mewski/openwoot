use super::{
    InputReport,
    descriptor::{Builder, Collection, MainFlags},
    usage::page,
};

pub const DESCRIPTOR: &[u8] = &descriptor().to_array::<{ descriptor().len() }>();
pub const ENTRIES: usize = 16;
pub const LEN: usize = ENTRIES * ENTRY_LEN;

const ENTRY_LEN: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Keyboard,
    Consumer,
    Function,
    Gamepad,
    Advanced,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Travel(u16);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Entry {
    pub key: u8,
    pub code: u8,
    pub pressed: bool,
    pub kind: Kind,
    pub travel: Travel,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    entries: [Entry; ENTRIES],
    len: usize,
}

impl Kind {
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Keyboard => 0,
            Self::Consumer => 3,
            Self::Function => 4,
            Self::Gamepad => 5,
            Self::Advanced => 6,
        }
    }
}

impl Travel {
    pub const MAX: Self = Self(0x3FF);

    #[must_use]
    pub const fn new(travel: u16) -> Option<Self> {
        if travel <= Self::MAX.0 {
            Some(Self(travel))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Entry {
    #[must_use]
    pub const fn to_bytes(self) -> [u8; ENTRY_LEN] {
        let state = self.pressed as u16 | (self.kind.value() as u16) << 2 | self.travel.get() << 6;
        let [state_low, state_high] = state.to_le_bytes();
        [self.key, self.code, state_low, state_high]
    }
}

impl Report {
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..self.len).unwrap_or_default()
    }

    pub fn push(&mut self, entry: Entry) -> bool {
        let Some(slot) = self.entries.get_mut(self.len) else {
            return false;
        };
        *slot = entry;
        self.len += 1;
        true
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

impl InputReport for Report {
    type Bytes = [u8; LEN];

    const ID: Option<u8> = None;
    const LEN: usize = LEN;

    fn to_bytes(&self) -> Self::Bytes {
        let mut bytes = [0; LEN];
        for (chunk, entry) in bytes
            .as_chunks_mut::<ENTRY_LEN>()
            .0
            .iter_mut()
            .zip(self.entries())
        {
            *chunk = entry.to_bytes();
        }
        bytes
    }
}

const fn descriptor() -> Builder {
    Builder::new()
        .usage_page(page::VENDOR_KEY_STATE)
        .usage(0x01)
        .collection(Collection::Application)
        .usage(0x02)
        .logical_minimum(0)
        .logical_maximum(0xFF)
        .report_size(8)
        .report_count(LEN)
        .input(MainFlags::DATA.variable())
        .end_collection()
}
