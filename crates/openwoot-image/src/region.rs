#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub start: u32,
    pub end: u32,
}

impl Region {
    #[must_use]
    pub const fn size(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn contains(self, address: u32) -> bool {
        self.start <= address && address < self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_holds_its_start_and_not_its_end() {
        let region = Region {
            start: 0x10,
            end: 0x20,
        };
        assert!(region.contains(0x10) && region.contains(0x1F));
        assert!(!region.contains(0x0F) && !region.contains(0x20));
        assert_eq!(region.size(), 0x10);
    }

    #[test]
    fn an_empty_or_inverted_region_holds_nothing() {
        for region in [
            Region {
                start: 0x10,
                end: 0x10,
            },
            Region {
                start: 0x20,
                end: 0x10,
            },
        ] {
            assert_eq!(region.size(), 0);
            assert!(!region.contains(0x10) && !region.contains(0x18));
        }
    }
}
