use std::sync::Arc;

use gpui_kit::{DisplayId, Pixels, Point, RenderImage, Size};
use moxcms::{CmsError, ColorProfile, Layout, RenderingIntent, TransformOptions};
use openwoot_protocol::lighting::Rgb;
use xcap::{
    Monitor,
    image::{Frame, RgbaImage},
};

use super::{Error, desktop};

const BYTES_PER_PIXEL: usize = 4;

pub(super) struct Screen {
    display: u32,
    image: Arc<RenderImage>,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Texel {
    column: u32,
    row: u32,
}

impl Screen {
    pub(super) fn capture() -> Result<Vec<Self>, Error> {
        Monitor::all()?.iter().map(Self::of).collect()
    }

    fn of(monitor: &Monitor) -> Result<Self, Error> {
        let display = monitor.id()?;
        let pixels = monitor.capture_image()?;
        let pixels = match desktop::profile(display) {
            Some(profile) => srgb_of(&pixels, &profile)?,
            None => pixels,
        };
        Ok(Self::from_pixels(display, pixels))
    }

    fn from_pixels(display: u32, mut pixels: RgbaImage) -> Self {
        for pixel in pixels.pixels_mut() {
            let [red, green, blue, alpha] = pixel.0;
            pixel.0 = [blue, green, red, alpha];
        }
        let (width, height) = pixels.dimensions();
        Self {
            display,
            image: Arc::new(RenderImage::new([Frame::new(pixels)])),
            width,
            height,
        }
    }

    pub(super) fn shows(&self, display: DisplayId) -> bool {
        u64::from(display) == u64::from(self.display)
    }

    pub(super) fn image(&self) -> Arc<RenderImage> {
        Arc::clone(&self.image)
    }

    pub(super) fn texel(&self, position: Point<Pixels>, viewport: Size<Pixels>) -> Texel {
        Texel {
            column: scale(position.x, viewport.width, self.width),
            row: scale(position.y, viewport.height, self.height),
        }
    }

    pub(super) fn color(&self, texel: Texel) -> Option<Rgb> {
        if texel.column >= self.width || texel.row >= self.height {
            return None;
        }
        let index = u64::from(texel.row) * u64::from(self.width) + u64::from(texel.column);
        let start = usize::try_from(index).ok()?.checked_mul(BYTES_PER_PIXEL)?;
        let end = start.checked_add(BYTES_PER_PIXEL)?;
        let pixel: [u8; BYTES_PER_PIXEL] =
            self.image.as_bytes(0)?.get(start..end)?.try_into().ok()?;
        let [blue, green, red, _] = pixel;
        Some(Rgb { red, green, blue })
    }

    pub(super) fn color_beside(&self, texel: Texel, columns: i32, rows: i32) -> Option<Rgb> {
        self.color(Texel {
            column: texel.column.checked_add_signed(columns)?,
            row: texel.row.checked_add_signed(rows)?,
        })
    }
}

fn srgb_of(pixels: &RgbaImage, profile: &[u8]) -> Result<RgbaImage, CmsError> {
    let transform = ColorProfile::new_from_slice(profile)?.create_transform_8bit(
        Layout::Rgba,
        &ColorProfile::new_srgb(),
        Layout::Rgba,
        TransformOptions {
            rendering_intent: RenderingIntent::RelativeColorimetric,
            ..TransformOptions::default()
        },
    )?;
    let mut converted = RgbaImage::new(pixels.width(), pixels.height());
    transform.transform(pixels, &mut converted)?;
    Ok(converted)
}

fn scale(offset: Pixels, length: Pixels, count: u32) -> u32 {
    let fraction = f64::from(f32::from(offset)) / f64::from(f32::from(length));
    let last = f64::from(count.saturating_sub(1));
    let scaled = (fraction * f64::from(count)).floor().clamp(0.0, last);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is floored and clamped to the image"
    )]
    let texel = scaled as u32;
    texel
}

#[cfg(test)]
mod tests {
    use gpui_kit::{point, px, size};
    use xcap::image::Rgba;

    use super::*;

    fn screen() -> Screen {
        let pixels = RgbaImage::from_fn(4, 2, |column, row| {
            let level = u8::try_from(column * 10 + row).unwrap_or(u8::MAX);
            Rgba([level, 100, 200, 255])
        });
        Screen::from_pixels(7, pixels)
    }

    #[test]
    fn positions_map_to_the_texel_under_them() {
        let screen = screen();
        let viewport = size(px(2.0), px(1.0));
        assert_eq!(
            screen.texel(point(px(0.0), px(0.0)), viewport),
            Texel { column: 0, row: 0 }
        );
        assert_eq!(
            screen.texel(point(px(1.6), px(0.6)), viewport),
            Texel { column: 3, row: 1 }
        );
        assert_eq!(
            screen.texel(point(px(5.0), px(-1.0)), viewport),
            Texel { column: 3, row: 0 }
        );
    }

    #[test]
    fn colors_come_back_in_capture_order() {
        let screen = screen();
        let texel = Texel { column: 2, row: 1 };
        assert_eq!(
            screen.color(texel),
            Some(Rgb {
                red: 21,
                green: 100,
                blue: 200,
            })
        );
        assert_eq!(
            screen.color_beside(texel, -2, -1),
            Some(Rgb {
                red: 0,
                green: 100,
                blue: 200,
            })
        );
        assert_eq!(screen.color_beside(texel, 2, 0), None);
        assert_eq!(screen.color_beside(texel, 0, -2), None);
    }

    #[test]
    fn a_screen_matches_only_its_own_display() {
        let screen = screen();
        assert!(screen.shows(DisplayId::new(7)));
        assert!(!screen.shows(DisplayId::new(8)));
    }
}
