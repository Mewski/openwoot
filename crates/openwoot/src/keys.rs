#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Index<const KEY_COUNT: usize>(usize);

impl<const KEY_COUNT: usize> Index<KEY_COUNT> {
    #[must_use]
    pub const fn new(index: usize) -> Option<Self> {
        if index < KEY_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}
