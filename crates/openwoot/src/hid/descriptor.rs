const CAPACITY: usize = 192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Builder {
    bytes: [u8; CAPACITY],
    len: usize,
    depth: u8,
    malformed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Collection {
    Physical,
    Application,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MainFlags(u8);

impl Builder {
    #[must_use]
    pub(super) const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            len: 0,
            depth: 0,
            malformed: false,
        }
    }

    #[must_use]
    pub(super) const fn usage_page(self, page: u16) -> Self {
        self.unsigned(Item::UsagePage, page as u32)
    }

    #[must_use]
    pub(super) const fn usage(self, usage: u16) -> Self {
        self.unsigned(Item::Usage, usage as u32)
    }

    #[must_use]
    pub(super) const fn usage_minimum(self, usage: u16) -> Self {
        self.unsigned(Item::UsageMinimum, usage as u32)
    }

    #[must_use]
    pub(super) const fn usage_maximum(self, usage: u16) -> Self {
        self.unsigned(Item::UsageMaximum, usage as u32)
    }

    #[must_use]
    pub(super) const fn logical_minimum(self, value: i32) -> Self {
        self.signed(Item::LogicalMinimum, value)
    }

    #[must_use]
    pub(super) const fn logical_maximum(self, value: i32) -> Self {
        self.signed(Item::LogicalMaximum, value)
    }

    #[must_use]
    pub(super) const fn physical_minimum(self, value: i32) -> Self {
        self.signed(Item::PhysicalMinimum, value)
    }

    #[must_use]
    pub(super) const fn physical_maximum(self, value: i32) -> Self {
        self.signed(Item::PhysicalMaximum, value)
    }

    #[must_use]
    pub(super) const fn unit(self, unit: u32) -> Self {
        self.unsigned(Item::Unit, unit)
    }

    #[must_use]
    pub(super) const fn report_id(self, report_id: u8) -> Self {
        if report_id == 0 {
            self.malformed()
        } else {
            self.unsigned(Item::ReportId, report_id as u32)
        }
    }

    #[must_use]
    pub(super) const fn report_size(self, bits: usize) -> Self {
        self.length(Item::ReportSize, bits)
    }

    #[must_use]
    pub(super) const fn report_count(self, count: usize) -> Self {
        self.length(Item::ReportCount, count)
    }

    #[must_use]
    pub(super) const fn input(self, flags: MainFlags) -> Self {
        self.unsigned(Item::Input, flags.0 as u32)
    }

    #[must_use]
    pub(super) const fn output(self, flags: MainFlags) -> Self {
        self.unsigned(Item::Output, flags.0 as u32)
    }

    #[must_use]
    pub(super) const fn feature(self, flags: MainFlags) -> Self {
        self.unsigned(Item::Feature, flags.0 as u32)
    }

    #[must_use]
    pub(super) const fn collection(mut self, collection: Collection) -> Self {
        self.depth += 1;
        let kind = match collection {
            Collection::Physical => 0,
            Collection::Application => 1,
        };
        self.unsigned(Item::Collection, kind)
    }

    #[must_use]
    pub(super) const fn end_collection(mut self) -> Self {
        if self.depth == 0 {
            return self.malformed();
        }
        self.depth -= 1;
        self.byte(Item::EndCollection.prefix())
    }

    #[must_use]
    pub(super) const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub(super) const fn to_array<const LEN: usize>(self) -> [u8; LEN] {
        assert!(
            !self.malformed && self.depth == 0 && self.len == LEN,
            "report descriptor is malformed or does not fit"
        );
        let mut bytes = [0; LEN];
        if let Some((written, _)) = self.bytes.split_at_checked(LEN) {
            bytes.copy_from_slice(written);
        }
        bytes
    }

    const fn unsigned(self, item: Item, value: u32) -> Self {
        let [low, second, third, high] = value.to_le_bytes();
        if value <= 0xFF {
            self.byte(item.prefix() | 1).byte(low)
        } else if value <= 0xFFFF {
            self.byte(item.prefix() | 2).byte(low).byte(second)
        } else {
            self.byte(item.prefix() | 3)
                .byte(low)
                .byte(second)
                .byte(third)
                .byte(high)
        }
    }

    const fn length(self, item: Item, value: usize) -> Self {
        match value.to_le_bytes() {
            [low, 0, ..] if value <= 0xFF => self.byte(item.prefix() | 1).byte(low),
            [low, high, ..] if value <= 0xFFFF => self.byte(item.prefix() | 2).byte(low).byte(high),
            _ => self.malformed(),
        }
    }

    const fn signed(self, item: Item, value: i32) -> Self {
        let [low, second, third, high] = value.to_le_bytes();
        if value >= i8::MIN as i32 && value <= i8::MAX as i32 {
            self.byte(item.prefix() | 1).byte(low)
        } else if value >= i16::MIN as i32 && value <= i16::MAX as i32 {
            self.byte(item.prefix() | 2).byte(low).byte(second)
        } else {
            self.byte(item.prefix() | 3)
                .byte(low)
                .byte(second)
                .byte(third)
                .byte(high)
        }
    }

    const fn byte(mut self, value: u8) -> Self {
        if let Some((_, free)) = self.bytes.split_at_mut_checked(self.len)
            && let [slot, ..] = free
        {
            *slot = value;
            self.len += 1;
            self
        } else {
            self.malformed()
        }
    }

    const fn malformed(mut self) -> Self {
        self.malformed = true;
        self
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl MainFlags {
    pub(super) const CONSTANT: Self = Self(0x01);
    pub(super) const DATA: Self = Self(0x00);

    #[must_use]
    pub(super) const fn variable(self) -> Self {
        Self(self.0 | 0x02)
    }

    #[must_use]
    pub(super) const fn relative(self) -> Self {
        Self(self.0 | 0x04)
    }

    #[must_use]
    pub(super) const fn null_state(self) -> Self {
        Self(self.0 | 0x40)
    }
}

#[derive(Clone, Copy)]
enum Item {
    Input,
    Output,
    Feature,
    Collection,
    EndCollection,
    UsagePage,
    LogicalMinimum,
    LogicalMaximum,
    PhysicalMinimum,
    PhysicalMaximum,
    Unit,
    ReportSize,
    ReportId,
    ReportCount,
    Usage,
    UsageMinimum,
    UsageMaximum,
}

impl Item {
    const fn prefix(self) -> u8 {
        match self {
            Self::Input => 0x80,
            Self::Output => 0x90,
            Self::Feature => 0xB0,
            Self::Collection => 0xA0,
            Self::EndCollection => 0xC0,
            Self::UsagePage => 0x04,
            Self::LogicalMinimum => 0x14,
            Self::LogicalMaximum => 0x24,
            Self::PhysicalMinimum => 0x34,
            Self::PhysicalMaximum => 0x44,
            Self::Unit => 0x64,
            Self::ReportSize => 0x74,
            Self::ReportId => 0x84,
            Self::ReportCount => 0x94,
            Self::Usage => 0x08,
            Self::UsageMinimum => 0x18,
            Self::UsageMaximum => 0x28,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitted(builder: &Builder) -> &[u8] {
        &builder.bytes[..builder.len]
    }

    #[test]
    fn a_signed_item_takes_the_smallest_size_that_keeps_its_sign() {
        let item = |value| Builder::new().logical_maximum(value);
        assert_eq!(emitted(&item(0)), [0x25, 0x00]);
        assert_eq!(emitted(&item(127)), [0x25, 0x7F]);
        assert_eq!(emitted(&item(128)), [0x26, 0x80, 0x00]);
        assert_eq!(emitted(&item(255)), [0x26, 0xFF, 0x00]);
        assert_eq!(emitted(&item(-128)), [0x25, 0x80]);
        assert_eq!(emitted(&item(-129)), [0x26, 0x7F, 0xFF]);
        assert_eq!(emitted(&item(32_767)), [0x26, 0xFF, 0x7F]);
        assert_eq!(emitted(&item(32_768)), [0x27, 0x00, 0x80, 0x00, 0x00]);
        assert_eq!(emitted(&item(-32_768)), [0x26, 0x00, 0x80]);
        assert_eq!(emitted(&item(-32_769)), [0x27, 0xFF, 0x7F, 0xFF, 0xFF]);
        assert_eq!(emitted(&item(i32::MIN)), [0x27, 0x00, 0x00, 0x00, 0x80]);
    }

    #[test]
    fn an_unsigned_item_takes_the_smallest_size_that_holds_its_value() {
        let item = |value| Builder::new().unit(value);
        assert_eq!(emitted(&item(0xFF)), [0x65, 0xFF]);
        assert_eq!(emitted(&item(0x100)), [0x66, 0x00, 0x01]);
        assert_eq!(emitted(&item(0xFFFF)), [0x66, 0xFF, 0xFF]);
        assert_eq!(emitted(&item(0x1_0000)), [0x67, 0x00, 0x00, 0x01, 0x00]);
        assert_eq!(
            emitted(&Builder::new().usage_page(0xFF53)),
            [0x06, 0x53, 0xFF]
        );
        assert_eq!(emitted(&Builder::new().usage(0x80)), [0x09, 0x80]);
    }

    #[test]
    fn a_count_above_two_bytes_is_malformed() {
        let item = |count| Builder::new().report_count(count);
        assert_eq!(emitted(&item(255)), [0x95, 0xFF]);
        assert_eq!(emitted(&item(256)), [0x96, 0x00, 0x01]);
        assert_eq!(emitted(&item(65_535)), [0x96, 0xFF, 0xFF]);
        assert!(!item(65_535).malformed);
        assert!(item(65_536).malformed);
        assert_eq!(item(65_536).len, 0);
    }

    #[test]
    fn structure_errors_and_overflow_are_malformed() {
        assert!(Builder::new().end_collection().malformed);
        assert!(Builder::new().report_id(0).malformed);
        assert!(!Builder::new().report_id(255).malformed);
        let open = Builder::new().collection(Collection::Application);
        assert_eq!((open.depth, open.malformed), (1, false));
        let mut full = Builder::new();
        for _ in 0..CAPACITY / 2 {
            full = full.usage(1);
        }
        assert_eq!((full.len, full.malformed), (CAPACITY, false));
        assert!(full.usage(1).malformed);
    }
}
