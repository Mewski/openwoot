use super::{
    color::{rgb, scale},
    config::Rgb,
    dirty_pages::DirtyPages,
    frame::Frame,
    leds::{Gains, Led, LedMap, Variant},
};

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
            self.store(led, scale(color, factor));
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
    use crate::{
        keys::Key,
        lighting::leds::{FramePage, LedPosition},
    };

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
}
