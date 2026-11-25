#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolKind {
    Function,
    Object,
    CodeStart,
    LiteralsStart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Symbol {
    pub(crate) name: String,
    pub(crate) address: u32,
    pub(crate) size: u32,
    pub(crate) kind: SymbolKind,
}

impl Symbol {
    pub(crate) const fn is_marker(&self) -> bool {
        matches!(self.kind, SymbolKind::CodeStart | SymbolKind::LiteralsStart)
    }

    pub(crate) fn contains(&self, address: u32) -> bool {
        address >= self.address && address - self.address < self.size
    }
}
