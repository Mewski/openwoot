mod channel;
mod controls;
mod descriptor;
mod gamepad;
mod key_state;
mod keyboard;
mod placeholder;
mod usage;
mod xinput;

pub use channel::{
    DESCRIPTOR as CHANNEL_DESCRIPTOR, FEATURE_REPORT_ID as CHANNEL_FEATURE_REPORT_ID,
    FEATURE_REPORT_LEN as CHANNEL_FEATURE_REPORT_LEN,
};
pub use controls::{
    CONSUMER_REPORT_ID, CONSUMER_SLOTS, ConsumerReport, DESCRIPTOR as CONTROLS_DESCRIPTOR,
    MOUSE_BUTTONS, MOUSE_REPORT_ID, MouseReport, SYSTEM_REPORT_ID, SystemControl, SystemReport,
};
pub use descriptor::{Builder as DescriptorBuilder, Collection, MainFlags};
pub use gamepad::{
    BUTTONS as GAMEPAD_BUTTONS, DESCRIPTOR as GAMEPAD_DESCRIPTOR, Hat, Report as GamepadReport,
    SLIDER_MAX as GAMEPAD_SLIDER_MAX, STICK_CENTER as GAMEPAD_STICK_CENTER,
    STICK_MAX as GAMEPAD_STICK_MAX,
};
pub use key_state::{
    DESCRIPTOR as KEY_STATE_DESCRIPTOR, ENTRIES as KEY_STATE_ENTRIES, Entry as KeyStateEntry,
    Kind as KeyStateKind, LEN as KEY_STATE_LEN, Report as KeyStateReport, Travel,
};
pub use keyboard::{
    BOOT_DESCRIPTOR as BOOT_KEYBOARD_DESCRIPTOR, BOOT_KEY_SLOTS, BootReport, KeyGroup, Leds,
    Modifiers, NKRO_BITMAP_LEN, NKRO_DESCRIPTOR, NKRO_KEY_GROUPS, NkroReport,
};
pub use placeholder::{DESCRIPTOR as PLACEHOLDER_DESCRIPTOR, LEN as PLACEHOLDER_LEN};
pub use xinput::{
    Buttons as XinputButtons, LEN as XINPUT_REPORT_LEN, Report as XinputReport, Stick,
};

pub trait InputReport {
    const ID: Option<u8>;
    const LEN: usize;

    type Bytes: AsRef<[u8]>;

    fn to_bytes(&self) -> Self::Bytes;
}
