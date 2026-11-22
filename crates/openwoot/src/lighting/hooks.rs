use super::{canvas::Canvas, config::Rgb};

pub trait Lightbar {
    fn needs_render(&self, since_render_ms: u16) -> bool {
        let _ = since_render_ms;
        false
    }

    fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u16) {
        let _ = (canvas, elapsed_ms);
    }

    fn request_redraw(&mut self) {}

    fn brightness(&self) -> u8 {
        0
    }

    fn apply_brightness_change(&mut self, key_brightness: u8) {
        let _ = key_brightness;
    }

    fn apply_page_brightness(&self, page: usize, linear: &[u8], output: &mut [u8], fade_level: u8) {
        let _ = (page, linear, output, fade_level);
    }

    fn owns_global_overlays(&self) -> bool {
        false
    }

    fn draw_overlay(&mut self, canvas: &mut impl Canvas, colors: &[Rgb]) {
        let _ = (canvas, colors);
    }

    fn clear_overlay(&mut self, canvas: &mut impl Canvas) {
        let _ = canvas;
    }
}

pub trait Signals {
    fn brightness_changed(&mut self, percent: f32) {
        let _ = percent;
    }
}

impl Lightbar for () {}

impl Signals for () {}
