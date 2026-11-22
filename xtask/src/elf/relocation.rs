#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelocationKind {
    Absolute,
    Branch,
    ConditionalBranch,
    MoveLow,
    MoveHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Relocation {
    pub(crate) offset: u32,
    pub(crate) kind: RelocationKind,
}
