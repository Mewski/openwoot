#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyPages<const PAGES: usize> {
    dirty: [bool; PAGES],
    next: usize,
}

impl<const PAGES: usize> Default for DirtyPages<PAGES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PAGES: usize> DirtyPages<PAGES> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dirty: [false; PAGES],
            next: 0,
        }
    }

    pub fn push(&mut self, page: usize) {
        if let Some(dirty) = self.dirty.get_mut(page) {
            *dirty = true;
        }
    }

    pub fn push_all(&mut self) {
        self.dirty = [true; PAGES];
    }

    pub fn pop(&mut self) -> Option<usize> {
        let page = (self.next..PAGES)
            .chain(0..self.next)
            .find(|page| self.dirty.get(*page) == Some(&true))?;
        if let Some(dirty) = self.dirty.get_mut(page) {
            *dirty = false;
        }
        self.next = page + 1;
        Some(page)
    }

    pub fn requeue(&mut self, page: usize) {
        if let Some(dirty) = self.dirty.get_mut(page) {
            *dirty = true;
            self.next = page;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_go_out_in_turn_from_the_one_after_the_last_taken() {
        let mut dirty = DirtyPages::<4>::new();
        dirty.push(2);
        dirty.push(0);
        dirty.push(2);
        dirty.push(9);
        assert_eq!(dirty.pop(), Some(0));
        dirty.push(0);
        assert_eq!(dirty.pop(), Some(2));
        dirty.push_all();
        assert_eq!(dirty.pop(), Some(3));
        assert_eq!(dirty.pop(), Some(0));
        assert_eq!(dirty.pop(), Some(1));
        assert_eq!(dirty.pop(), Some(2));
        assert_eq!(dirty.pop(), None);
    }

    #[test]
    fn a_requeued_page_goes_out_first() {
        let mut dirty = DirtyPages::<4>::new();
        dirty.push_all();
        assert_eq!(dirty.pop(), Some(0));
        assert_eq!(dirty.pop(), Some(1));
        dirty.requeue(1);
        assert_eq!(dirty.pop(), Some(1));
        assert_eq!(dirty.pop(), Some(2));
        dirty.requeue(7);
        assert_eq!(dirty.pop(), Some(3));
        assert_eq!(dirty.pop(), None);
    }
}
