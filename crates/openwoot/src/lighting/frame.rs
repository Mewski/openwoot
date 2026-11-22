use super::{
    color::rgb,
    config::Rgb,
    dirty_pages::DirtyPages,
    leds::{FramePage, LedPosition},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame<const PAGES: usize, const LENGTH: usize> {
    pages: [[u8; LENGTH]; PAGES],
}

impl<const PAGES: usize, const LENGTH: usize> Default for Frame<PAGES, LENGTH> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PAGES: usize, const LENGTH: usize> Frame<PAGES, LENGTH> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pages: [[0; LENGTH]; PAGES],
        }
    }

    #[must_use]
    pub fn page(&self, page: usize) -> Option<&[u8; LENGTH]> {
        self.pages.get(page)
    }

    #[must_use]
    pub fn color(&self, position: LedPosition) -> Option<Rgb> {
        let offset = usize::from(position.offset);
        let channels = self
            .pages
            .get(usize::from(position.page))?
            .get(offset..offset + 3)?;
        match *channels {
            [red, green, blue] => Some(rgb(red, green, blue)),
            _ => None,
        }
    }

    pub(super) fn store(&mut self, position: LedPosition, color: Rgb) -> bool {
        let offset = usize::from(position.offset);
        let Some(channels) = self
            .pages
            .get_mut(usize::from(position.page))
            .and_then(|page| page.get_mut(offset..offset + 3))
        else {
            return false;
        };
        let stored = [color.red, color.green, color.blue];
        let changed = *channels != stored;
        channels.copy_from_slice(&stored);
        changed
    }

    pub(super) fn merge_into(
        &self,
        main: &mut Self,
        pages: &[FramePage; PAGES],
        dirty: &mut DirtyPages<PAGES>,
    ) {
        for (index, ((target, source), layout)) in main
            .pages
            .iter_mut()
            .zip(&self.pages)
            .zip(pages)
            .enumerate()
        {
            let length = usize::from(layout.length).min(LENGTH);
            let (Some(target), Some(source)) = (target.get_mut(..length), source.get(..length))
            else {
                continue;
            };
            if target != source {
                target.copy_from_slice(source);
                dirty.push(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{
        canvas::{Canvas, Painter},
        leds::{
            Led,
            tests::{MAP, VARIANT},
        },
    };

    #[test]
    fn merging_copies_only_the_page_runs_and_queues_changes() {
        let mut main = Frame::<2, 6>::new();
        let mut scratch = Frame::<2, 6>::new();
        let mut dirty = DirtyPages::new();
        Painter::new(&mut scratch, &MAP, &VARIANT, None).set_led(Led(3), rgb(10, 20, 30));
        scratch.pages[1][4] = 77;
        scratch.merge_into(&mut main, &MAP.pages, &mut dirty);
        assert_eq!(dirty.queued(), &[1]);
        assert_eq!(main.page(1), Some(&[9, 20, 30, 0, 0, 0]));
    }
}
