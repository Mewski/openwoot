use super::output::PageWrite;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SentPages<const PAGES: usize, const LENGTH: usize> {
    pages: [Option<[u8; LENGTH]>; PAGES],
}

impl<const PAGES: usize, const LENGTH: usize> SentPages<PAGES, LENGTH> {
    pub(super) const fn new() -> Self {
        Self {
            pages: [None; PAGES],
        }
    }

    pub(super) fn holds(&self, write: &PageWrite<LENGTH>) -> bool {
        self.pages
            .get(write.page)
            .and_then(Option::as_ref)
            .and_then(|sent| sent.get(..write.values().len()))
            == Some(write.values())
    }

    pub(super) fn record(&mut self, write: &PageWrite<LENGTH>) {
        if let Some(page) = self.pages.get_mut(write.page) {
            let mut sent = [0; LENGTH];
            for (sent, value) in sent.iter_mut().zip(write.values()) {
                *sent = *value;
            }
            *page = Some(sent);
        }
    }

    pub(super) const fn forget(&mut self) {
        self.pages = [None; PAGES];
    }
}
