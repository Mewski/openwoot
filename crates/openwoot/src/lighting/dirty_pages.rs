#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyPages<const PAGES: usize> {
    queue: [u8; PAGES],
    length: usize,
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
            queue: [0; PAGES],
            length: 0,
        }
    }

    pub fn push(&mut self, page: usize) {
        let Ok(page) = u8::try_from(page) else {
            return;
        };
        if usize::from(page) >= PAGES || self.queued().contains(&page) {
            return;
        }
        if let Some(slot) = self.queue.get_mut(self.length) {
            *slot = page;
            self.length += 1;
        }
    }

    pub fn push_all(&mut self) {
        for page in 0..PAGES {
            self.push(page);
        }
    }

    pub fn pop(&mut self) -> Option<usize> {
        let first = *self.queued().first()?;
        self.queue.copy_within(1..self.length, 0);
        self.length -= 1;
        Some(usize::from(first))
    }

    #[must_use]
    pub fn queued(&self) -> &[u8] {
        self.queue.get(..self.length).unwrap_or_default()
    }
}
