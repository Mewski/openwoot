use super::{
    color::{rgb, scale_channel},
    config::Rgb,
    gamma::output_value,
    leds::{FramePage, Gains, Led, LedMap, LedPosition, Variant},
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

    fn store(&mut self, position: LedPosition, color: Rgb) -> bool {
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

    pub(crate) fn merge_into(
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
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    #[must_use]
    pub fn queued(&self) -> &[u8] {
        self.queue.get(..self.length).unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageWrite<const LENGTH: usize> {
    pub page: usize,
    pub bank: u8,
    pub first_register: u8,
    values: [u8; LENGTH],
    length: usize,
}

impl<const LENGTH: usize> PageWrite<LENGTH> {
    pub(crate) fn new(page: usize, layout: FramePage, linear: &[u8; LENGTH], scale: u8) -> Self {
        let length = usize::from(layout.length).min(LENGTH);
        let mut values = [0; LENGTH];
        for (value, linear) in values.iter_mut().zip(linear).take(length) {
            *value = output_value(*linear, scale);
        }
        Self {
            page,
            bank: layout.bank,
            first_register: layout.first_register,
            values,
            length,
        }
    }

    #[must_use]
    pub fn values(&self) -> &[u8] {
        self.values.get(..self.length).unwrap_or_default()
    }

    pub fn values_mut(&mut self) -> &mut [u8] {
        self.values.get_mut(..self.length).unwrap_or_default()
    }
}

pub trait Canvas {
    fn led_of(&self, row: usize, column: usize) -> Option<Led>;

    fn set_led(&mut self, led: Led, color: Rgb);

    fn scale_led(&mut self, led: Led, factor: u8);

    fn led(&self, led: Led) -> Option<Rgb>;

    fn set_key(&mut self, row: usize, column: usize, color: Rgb) {
        if let Some(led) = self.led_of(row, column) {
            self.set_led(led, color);
        }
    }

    fn scale_key(&mut self, row: usize, column: usize, factor: u8) {
        if let Some(led) = self.led_of(row, column) {
            self.scale_led(led, factor);
        }
    }

    fn key(&self, row: usize, column: usize) -> Option<Rgb> {
        self.led_of(row, column).and_then(|led| self.led(led))
    }
}

pub struct Painter<
    'a,
    const ROWS: usize,
    const COLUMNS: usize,
    const PAGES: usize,
    const LENGTH: usize,
> {
    frame: &'a mut Frame<PAGES, LENGTH>,
    map: &'a LedMap<PAGES>,
    variant: &'a Variant<ROWS, COLUMNS>,
    dirty: Option<&'a mut DirtyPages<PAGES>>,
}

impl<'a, const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize>
    Painter<'a, ROWS, COLUMNS, PAGES, LENGTH>
{
    pub fn new(
        frame: &'a mut Frame<PAGES, LENGTH>,
        map: &'a LedMap<PAGES>,
        variant: &'a Variant<ROWS, COLUMNS>,
        dirty: Option<&'a mut DirtyPages<PAGES>>,
    ) -> Self {
        Self {
            frame,
            map,
            variant,
            dirty,
        }
    }

    fn store(&mut self, led: Led, color: Rgb) {
        let Some(position) = self.map.position(led) else {
            return;
        };
        if self.frame.store(position, color)
            && let Some(dirty) = self.dirty.as_deref_mut()
        {
            dirty.push(usize::from(position.page));
        }
    }
}

impl<const ROWS: usize, const COLUMNS: usize, const PAGES: usize, const LENGTH: usize> Canvas
    for Painter<'_, ROWS, COLUMNS, PAGES, LENGTH>
{
    fn led_of(&self, row: usize, column: usize) -> Option<Led> {
        self.variant.led(row, column)
    }

    fn set_led(&mut self, led: Led, color: Rgb) {
        let gains = self.map.gains;
        self.store(led, apply_gains(color, gains));
    }

    fn scale_led(&mut self, led: Led, factor: u8) {
        if let Some(color) = self.led(led) {
            self.store(
                led,
                rgb(
                    scale_channel(color.red, factor),
                    scale_channel(color.green, factor),
                    scale_channel(color.blue, factor),
                ),
            );
        }
    }

    fn led(&self, led: Led) -> Option<Rgb> {
        self.map
            .position(led)
            .and_then(|position| self.frame.color(position))
    }
}

const fn apply_gains(color: Rgb, gains: Gains) -> Rgb {
    rgb(
        gain(color.red, gains.red),
        gain(color.green, gains.green),
        gain(color.blue, gains.blue),
    )
}

const fn gain(value: u8, gain: u8) -> u8 {
    ((gain as u32 * (value as u32 + 1)) >> 8).to_le_bytes()[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIONS: [Option<LedPosition>; 4] = [
        None,
        Some(LedPosition::new(0, 0)),
        Some(LedPosition::new(0, 3)),
        Some(LedPosition::new(1, 0)),
    ];
    const MAP: LedMap<2> = LedMap {
        positions: &POSITIONS,
        pages: [FramePage::new(0, 0, 6), FramePage::new(1, 90, 3)],
        gains: Gains {
            red: 232,
            green: 255,
            blue: 255,
        },
    };
    const VARIANT: Variant<1, 3> = Variant {
        leds: [[Some(Led(1)), Some(Led(3)), None]],
        spacebar: Key::new(0, 0),
        spacebar_fill: &[],
    };

    use crate::lighting::leds::Key;

    #[test]
    fn gains_limit_the_stored_channels() {
        assert_eq!(gain(255, 255), 255);
        assert_eq!(gain(254, 255), 254);
        assert_eq!(gain(0, 255), 0);
        assert_eq!(gain(255, 232), 232);
        assert_eq!(gain(0, 232), 0);
    }

    #[test]
    fn writes_to_the_tracked_frame_queue_each_changed_page_once() {
        let mut frame = Frame::<2, 6>::new();
        let mut dirty = DirtyPages::new();
        let mut painter = Painter::new(&mut frame, &MAP, &VARIANT, Some(&mut dirty));
        painter.set_key(0, 1, rgb(1, 2, 3));
        painter.set_led(Led(1), rgb(255, 255, 255));
        painter.set_led(Led(2), rgb(0, 0, 0));
        painter.set_key(0, 2, rgb(9, 9, 9));
        painter.set_led(Led(9), rgb(9, 9, 9));
        assert_eq!(painter.key(0, 0), Some(rgb(232, 255, 255)));
        painter.scale_key(0, 0, 128);
        assert_eq!(painter.key(0, 0), Some(rgb(116, 128, 128)));
        assert_eq!(dirty.queued(), &[1, 0]);
        assert_eq!(dirty.pop(), Some(1));
        assert_eq!(dirty.pop(), Some(0));
        assert_eq!(dirty.pop(), None);
        assert_eq!(frame.page(0), Some(&[116, 128, 128, 0, 0, 0]));
    }

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

    #[test]
    fn page_writes_carry_gamma_corrected_scaled_values() {
        let write = PageWrite::new(8, FramePage::new(1, 90, 4), &[255, 128, 0, 255, 7, 7], 255);
        assert_eq!(write.bank, 1);
        assert_eq!(write.first_register, 90);
        assert_eq!(write.values(), &[255, 50, 0, 255]);
    }
}
