mod channel;
mod controls;
mod descriptor;
mod gamepad;
mod key_state;
mod keyboard;
mod placeholder;
mod usage;

pub use channel::{
    DESCRIPTOR as CHANNEL_DESCRIPTOR, FEATURE_REPORT_LEN as CHANNEL_FEATURE_REPORT_LEN,
};
pub use controls::{
    CONSUMER_SLOTS, ConsumerReport, DESCRIPTOR as CONTROLS_DESCRIPTOR, MouseReport, SystemControl,
    SystemReport,
};
pub use gamepad::{
    DESCRIPTOR as GAMEPAD_DESCRIPTOR, Hat, Report as GamepadReport,
    SLIDER_MAX as GAMEPAD_SLIDER_MAX, STICK_CENTER as GAMEPAD_STICK_CENTER,
    STICK_MAX as GAMEPAD_STICK_MAX,
};
pub use key_state::{
    DESCRIPTOR as KEY_STATE_DESCRIPTOR, ENTRIES as KEY_STATE_ENTRIES, Entry as KeyStateEntry,
    Kind as KeyStateKind, LEN as KEY_STATE_LEN, Report as KeyStateReport, Travel,
};
pub use keyboard::{
    BOOT_DESCRIPTOR as BOOT_KEYBOARD_DESCRIPTOR, BootReport, Leds, Modifiers, NKRO_DESCRIPTOR,
    NkroReport,
};
pub use placeholder::{DESCRIPTOR as PLACEHOLDER_DESCRIPTOR, LEN as PLACEHOLDER_LEN};

pub trait InputReport {
    const ID: Option<u8>;
    const LEN: usize;

    type Bytes: AsRef<[u8]>;

    fn to_bytes(&self) -> Self::Bytes;
}
