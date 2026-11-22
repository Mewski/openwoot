#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Changes(u16);

impl Changes {
    pub const ACTIVATED: Self = Self(1 << 4);
    pub const COLORS: Self = Self(1 << 1);
    pub const DEVICE: Self = Self(1 << 6);
    pub const DEVICE_WIDE: Self = Self(Self::GLOBAL.0 | Self::DEVICE.0 | Self::GLOBAL_LIGHTBAR.0);
    pub const GLOBAL: Self = Self(1 << 2);
    pub const GLOBAL_LIGHTBAR: Self = Self(1 << 8);
    pub const INFO: Self = Self(1 << 5);
    pub const LIGHTBAR: Self = Self(1 << 7);
    pub const LIGHTING: Self = Self(1 << 0);
    pub const NONE: Self = Self(0);
    pub const SCAN: Self = Self(1 << 3);

    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_union_contains_both_parts_and_nothing_else() {
        let changes = Changes::LIGHTING.union(Changes::COLORS);
        assert!(changes.contains(Changes::LIGHTING));
        assert!(changes.contains(Changes::COLORS));
        assert!(!changes.contains(Changes::GLOBAL));
        assert!(Changes::NONE.is_empty());
        assert_eq!(Changes::from_bits(changes.bits()), changes);
    }

    #[test]
    fn device_wide_changes_split_from_the_profile_changes() {
        let changes = Changes::LIGHTING
            .union(Changes::GLOBAL)
            .union(Changes::GLOBAL_LIGHTBAR)
            .union(Changes::LIGHTBAR);
        assert_eq!(
            changes.intersection(Changes::DEVICE_WIDE),
            Changes::GLOBAL.union(Changes::GLOBAL_LIGHTBAR)
        );
        assert_eq!(
            changes.without(Changes::DEVICE_WIDE),
            Changes::LIGHTING.union(Changes::LIGHTBAR)
        );
    }
}
