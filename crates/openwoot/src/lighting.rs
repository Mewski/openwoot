mod brightness;
mod canvas;
mod color;
mod dirty_pages;
mod effects;
mod engine;
mod frame;
mod hooks;
mod idle;
mod indicators;
mod input;
mod key_colors;
mod leds;
mod matrix;
mod output;
mod overlay;
mod truncate;

pub use brightness::{BRIGHTNESS_MAX_FAST_SCAN, BRIGHTNESS_SAVE_DELAY_MS, BRIGHTNESS_STEP};
pub use canvas::{Canvas, Painter};
pub use color::{
    BLACK, PALETTE, blend, from_rgb565, mix_level, rgb, scale, scale_channel, to_rgb565,
};
pub use dirty_pages::DirtyPages;
pub use effects::Layers;
pub use engine::{FRAME_PERIOD_MS, Lighting, OutputChange};
pub use frame::Frame;
pub use hooks::{Lightbar, Signals};
pub use idle::FADE_DIMMED;
pub use indicators::{Indicator, LockState};
pub use input::{KEY_EVENT_CAPACITY, KeyActivity, KeyEvent, KeyEvents, KeyTravel, TRAVEL_FULL};
pub use key_colors::{KeyColorMap, LAYER_COLOR_MAPS};
pub use leds::{FramePage, Gains, Geometry, Led, LedMap, LedPosition, Variant};
pub use matrix::{LedMatrix, Matrix};
use openwoot_protocol::lighting as config;
pub use output::{GAMMA, PageWrite, gamma, output_scale, output_value};
pub use overlay::{FADE_FULL, MAX_LIGHTBAR_LEDS, OVERLAY_SLOTS, Overlay, OverlayId, Span};
