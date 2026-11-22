use super::{
    InputReport,
    descriptor::{Builder, Collection, MainFlags},
    usage::{desktop, keyboard, led, page},
};

pub const BOOT_DESCRIPTOR: &[u8] = &boot_descriptor().to_array::<{ boot_descriptor().len() }>();
pub const NKRO_DESCRIPTOR: &[u8] = &nkro_descriptor().to_array::<{ nkro_descriptor().len() }>();
pub const BOOT_KEY_SLOTS: usize = 6;
pub const NKRO_BITMAP_LEN: usize = bitmap_len();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers(u8);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BootReport {
    pub modifiers: Modifiers,
    pub keys: [u8; BOOT_KEY_SLOTS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NkroReport {
    pub modifiers: Modifiers,
    bitmap: [u8; NKRO_BITMAP_LEN],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Leds(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyGroup {
    pub first: u8,
    pub last: u8,
}

pub const NKRO_KEY_GROUPS: [KeyGroup; 4] = [
    KeyGroup {
        first: 0x04,
        last: 0x31,
    },
    KeyGroup {
        first: 0x33,
        last: 0x9B,
    },
    KeyGroup {
        first: 0x9D,
        last: 0xA4,
    },
    KeyGroup {
        first: 0xB0,
        last: 0xDD,
    },
];

impl Modifiers {
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    pub fn set(&mut self, usage: u8, pressed: bool) -> bool {
        let Some(bit) = modifier_bit(usage) else {
            return false;
        };
        if pressed {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
        true
    }

    #[must_use]
    pub fn contains(self, usage: u8) -> bool {
        modifier_bit(usage).is_some_and(|bit| self.0 & bit != 0)
    }
}

impl BootReport {
    pub fn press(&mut self, usage: u8) -> bool {
        if self.modifiers.set(usage, true) || self.keys.contains(&usage) {
            return true;
        }
        match self.keys.iter_mut().find(|slot| **slot == 0) {
            Some(slot) => {
                *slot = usage;
                true
            }
            None => false,
        }
    }

    pub fn release(&mut self, usage: u8) {
        if !self.modifiers.set(usage, false) {
            self.keys
                .iter_mut()
                .filter(|slot| **slot == usage)
                .for_each(|slot| *slot = 0);
        }
    }
}

impl InputReport for BootReport {
    type Bytes = [u8; 8];

    const ID: Option<u8> = None;
    const LEN: usize = 8;

    fn to_bytes(&self) -> Self::Bytes {
        let [first, second, third, fourth, fifth, sixth] = self.keys;
        [
            self.modifiers.bits(),
            0,
            first,
            second,
            third,
            fourth,
            fifth,
            sixth,
        ]
    }
}

impl NkroReport {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            modifiers: Modifiers(0),
            bitmap: [0; NKRO_BITMAP_LEN],
        }
    }

    pub fn set(&mut self, usage: u8, pressed: bool) -> bool {
        if self.modifiers.set(usage, pressed) {
            return true;
        }
        let Some((byte, bit)) = bitmap_position(usage) else {
            return false;
        };
        let Some(bits) = self.bitmap.get_mut(byte) else {
            return false;
        };
        if pressed {
            *bits |= bit;
        } else {
            *bits &= !bit;
        }
        true
    }

    #[must_use]
    pub fn is_pressed(&self, usage: u8) -> bool {
        self.modifiers.contains(usage)
            || bitmap_position(usage).is_some_and(|(byte, bit)| {
                self.bitmap.get(byte).is_some_and(|bits| bits & bit != 0)
            })
    }
}

impl Default for NkroReport {
    fn default() -> Self {
        Self::new()
    }
}

impl InputReport for NkroReport {
    type Bytes = [u8; 1 + NKRO_BITMAP_LEN];

    const ID: Option<u8> = None;
    const LEN: usize = 1 + NKRO_BITMAP_LEN;

    fn to_bytes(&self) -> Self::Bytes {
        let mut bytes = [0; 1 + NKRO_BITMAP_LEN];
        if let Some((modifiers, bitmap)) = bytes.split_first_mut() {
            *modifiers = self.modifiers.bits();
            bitmap.copy_from_slice(&self.bitmap);
        }
        bytes
    }
}

impl Leds {
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x1F)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn num_lock(self) -> bool {
        self.0 & 0x01 != 0
    }

    #[must_use]
    pub const fn caps_lock(self) -> bool {
        self.0 & 0x02 != 0
    }

    #[must_use]
    pub const fn scroll_lock(self) -> bool {
        self.0 & 0x04 != 0
    }

    #[must_use]
    pub const fn compose(self) -> bool {
        self.0 & 0x08 != 0
    }

    #[must_use]
    pub const fn kana(self) -> bool {
        self.0 & 0x10 != 0
    }
}

impl KeyGroup {
    #[must_use]
    pub const fn key_count(self) -> usize {
        self.last as usize - self.first as usize + 1
    }

    #[must_use]
    pub const fn padding_bits(self) -> usize {
        self.key_count().next_multiple_of(8) - self.key_count()
    }
}

fn modifier_bit(usage: u8) -> Option<u8> {
    let offset = u16::from(usage).checked_sub(keyboard::LEFT_CONTROL)?;
    (u16::from(usage) <= keyboard::RIGHT_GUI).then(|| 1 << offset)
}

fn bitmap_position(usage: u8) -> Option<(usize, u8)> {
    let mut group_offset = 0;
    for group in NKRO_KEY_GROUPS {
        if (group.first..=group.last).contains(&usage) {
            let index = usize::from(usage - group.first);
            return Some((group_offset + index / 8, 1 << (index % 8)));
        }
        group_offset += group.key_count().div_ceil(8);
    }
    None
}

const fn bitmap_len() -> usize {
    let mut len = 0;
    let mut groups: &[KeyGroup] = &NKRO_KEY_GROUPS;
    while let [group, rest @ ..] = groups {
        len += group.key_count().div_ceil(8);
        groups = rest;
    }
    len
}

const fn modifiers(builder: Builder) -> Builder {
    builder
        .usage_page(page::KEYBOARD)
        .usage_minimum(keyboard::LEFT_CONTROL)
        .usage_maximum(keyboard::RIGHT_GUI)
        .logical_minimum(0)
        .logical_maximum(1)
        .report_size(1)
        .report_count(8)
        .input(MainFlags::DATA.variable())
}

const fn leds(builder: Builder) -> Builder {
    builder
        .usage_page(page::LED)
        .usage_minimum(led::NUM_LOCK)
        .usage_maximum(led::KANA)
        .logical_minimum(0)
        .logical_maximum(1)
        .report_size(1)
        .report_count(5)
        .output(MainFlags::DATA.variable())
}

const fn boot_descriptor() -> Builder {
    let builder = modifiers(
        Builder::new()
            .usage_page(page::GENERIC_DESKTOP)
            .usage(desktop::KEYBOARD)
            .collection(Collection::Application),
    )
    .input(MainFlags::CONSTANT)
    .usage_minimum(0)
    .usage_maximum(keyboard::LAST_KEY)
    .logical_minimum(0)
    .logical_maximum(keyboard::LAST_KEY as i32)
    .report_size(8)
    .report_count(BOOT_KEY_SLOTS)
    .input(MainFlags::DATA);
    leds(builder)
        .report_count(3)
        .output(MainFlags::CONSTANT)
        .end_collection()
}

const fn nkro_descriptor() -> Builder {
    let mut builder = modifiers(
        leds(
            Builder::new()
                .usage_page(page::GENERIC_DESKTOP)
                .usage(desktop::KEYBOARD)
                .collection(Collection::Application),
        )
        .report_size(3)
        .report_count(1)
        .output(MainFlags::CONSTANT),
    );
    let mut groups: &[KeyGroup] = &NKRO_KEY_GROUPS;
    while let [group, rest @ ..] = groups {
        builder = builder
            .usage_minimum(group.first as u16)
            .usage_maximum(group.last as u16)
            .report_size(1)
            .report_count(group.key_count())
            .input(MainFlags::DATA.variable());
        if group.padding_bits() > 0 {
            builder = builder
                .report_size(group.padding_bits())
                .report_count(1)
                .input(MainFlags::CONSTANT);
        }
        groups = rest;
    }
    builder.end_collection()
}
