mod color;
mod effects;
mod frame;
mod gamma;
mod hooks;
mod input;
mod leds;
mod output;
mod overlay;
mod renderer;

pub use color::{
    BLACK, PALETTE, blend, from_rgb565, mix_level, rgb, scale, scale_channel, to_rgb565,
};
pub use effects::Layers;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
pub use frame::{Canvas, DirtyPages, Frame, PageWrite, Painter};
pub use gamma::{GAMMA, gamma, output_scale, output_value};
pub use hooks::{Lightbar, Signals};
pub use input::{KeyEvent, KeyTravel, TRAVEL_FULL};
pub use leds::{FramePage, Gains, Geometry, Key, Led, LedMap, LedPosition, Variant};
use openwoot_protocol::lighting as config;
pub use output::{LedMatrix, Matrix};
pub use overlay::{MAX_LIGHTBAR_LEDS, OVERLAY_SLOTS, Overlay, OverlayId, Span};
pub use renderer::{
    BRIGHTNESS_MAX_FAST_SCAN, BRIGHTNESS_SAVE_DELAY_MS, BRIGHTNESS_STEP, FADE_DIMMED, FADE_FULL,
    FRAME_PERIOD_MS, Indicator, KeyColorMap, LAYER_COLOR_MAPS, Lighting, LockState, OutputChange,
};

pub const KEY_EVENT_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyActivity {
    pub key: Key,
    pub event: KeyEvent,
    pub at_ms: u32,
}

pub type KeyEvents = Channel<CriticalSectionRawMutex, KeyActivity, KEY_EVENT_CAPACITY>;
