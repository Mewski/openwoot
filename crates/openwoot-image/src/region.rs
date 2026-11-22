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
