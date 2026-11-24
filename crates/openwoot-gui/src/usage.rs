use std::borrow::Cow;

use openwoot_protocol::{
    gamepad::Control,
    keys::{Action, Function, Layer, MouseButton, SystemControl},
};

pub(crate) const KEYBOARD: &[(u8, &str, &str)] = &[
    (0x04, "A", "A"),
    (0x05, "B", "B"),
    (0x06, "C", "C"),
    (0x07, "D", "D"),
    (0x08, "E", "E"),
    (0x09, "F", "F"),
    (0x0A, "G", "G"),
    (0x0B, "H", "H"),
    (0x0C, "I", "I"),
    (0x0D, "J", "J"),
    (0x0E, "K", "K"),
    (0x0F, "L", "L"),
    (0x10, "M", "M"),
    (0x11, "N", "N"),
    (0x12, "O", "O"),
    (0x13, "P", "P"),
    (0x14, "Q", "Q"),
    (0x15, "R", "R"),
    (0x16, "S", "S"),
    (0x17, "T", "T"),
    (0x18, "U", "U"),
    (0x19, "V", "V"),
    (0x1A, "W", "W"),
    (0x1B, "X", "X"),
    (0x1C, "Y", "Y"),
    (0x1D, "Z", "Z"),
    (0x1E, "1", "1 and !"),
    (0x1F, "2", "2 and @"),
    (0x20, "3", "3 and #"),
    (0x21, "4", "4 and $"),
    (0x22, "5", "5 and %"),
    (0x23, "6", "6 and ^"),
    (0x24, "7", "7 and &"),
    (0x25, "8", "8 and *"),
    (0x26, "9", "9 and ("),
    (0x27, "0", "0 and )"),
    (0x28, "Enter", "Enter"),
    (0x29, "Esc", "Escape"),
    (0x2A, "Bksp", "Backspace"),
    (0x2B, "Tab", "Tab"),
    (0x2C, "Space", "Space"),
    (0x2D, "-", "Minus"),
    (0x2E, "=", "Equals"),
    (0x2F, "[", "Left bracket"),
    (0x30, "]", "Right bracket"),
    (0x31, "\\", "Backslash"),
    (0x33, ";", "Semicolon"),
    (0x34, "'", "Apostrophe"),
    (0x35, "`", "Grave accent"),
    (0x36, ",", "Comma"),
    (0x37, ".", "Period"),
    (0x38, "/", "Slash"),
    (0x39, "Caps", "Caps Lock"),
    (0x3A, "F1", "F1"),
    (0x3B, "F2", "F2"),
    (0x3C, "F3", "F3"),
    (0x3D, "F4", "F4"),
    (0x3E, "F5", "F5"),
    (0x3F, "F6", "F6"),
    (0x40, "F7", "F7"),
    (0x41, "F8", "F8"),
    (0x42, "F9", "F9"),
    (0x43, "F10", "F10"),
    (0x44, "F11", "F11"),
    (0x45, "F12", "F12"),
    (0x46, "PrtSc", "Print Screen"),
    (0x47, "ScrLk", "Scroll Lock"),
    (0x48, "Pause", "Pause"),
    (0x49, "Ins", "Insert"),
    (0x4A, "Home", "Home"),
    (0x4B, "PgUp", "Page Up"),
    (0x4C, "Del", "Delete"),
    (0x4D, "End", "End"),
    (0x4E, "PgDn", "Page Down"),
    (0x4F, "→", "Right arrow"),
    (0x50, "←", "Left arrow"),
    (0x51, "↓", "Down arrow"),
    (0x52, "↑", "Up arrow"),
    (0x53, "Num", "Num Lock"),
    (0x54, "KP /", "Keypad divide"),
    (0x55, "KP *", "Keypad multiply"),
    (0x56, "KP -", "Keypad minus"),
    (0x57, "KP +", "Keypad plus"),
    (0x58, "KP ↵", "Keypad Enter"),
    (0x59, "KP 1", "Keypad 1"),
    (0x5A, "KP 2", "Keypad 2"),
    (0x5B, "KP 3", "Keypad 3"),
    (0x5C, "KP 4", "Keypad 4"),
    (0x5D, "KP 5", "Keypad 5"),
    (0x5E, "KP 6", "Keypad 6"),
    (0x5F, "KP 7", "Keypad 7"),
    (0x60, "KP 8", "Keypad 8"),
    (0x61, "KP 9", "Keypad 9"),
    (0x62, "KP 0", "Keypad 0"),
    (0x63, "KP .", "Keypad period"),
    (0x64, "ISO \\", "ISO backslash"),
    (0x65, "Menu", "Application menu"),
    (0x66, "Power", "Power"),
    (0x67, "KP =", "Keypad equals"),
    (0x68, "F13", "F13"),
    (0x69, "F14", "F14"),
    (0x6A, "F15", "F15"),
    (0x6B, "F16", "F16"),
    (0x6C, "F17", "F17"),
    (0x6D, "F18", "F18"),
    (0x6E, "F19", "F19"),
    (0x6F, "F20", "F20"),
    (0x70, "F21", "F21"),
    (0x71, "F22", "F22"),
    (0x72, "F23", "F23"),
    (0x73, "F24", "F24"),
    (0x74, "Exec", "Execute"),
    (0x75, "Help", "Help"),
    (0x76, "Menu", "Menu"),
    (0x77, "Select", "Select"),
    (0x78, "Stop", "Stop"),
    (0x79, "Again", "Again"),
    (0x7A, "Undo", "Undo"),
    (0x7B, "Cut", "Cut"),
    (0x7C, "Copy", "Copy"),
    (0x7D, "Paste", "Paste"),
    (0x7E, "Find", "Find"),
    (0x7F, "Mute", "Mute"),
    (0x80, "Vol +", "Volume up"),
    (0x81, "Vol -", "Volume down"),
    (0x82, "LCaps", "Locking Caps Lock"),
    (0x83, "LNum", "Locking Num Lock"),
    (0x84, "LScr", "Locking Scroll Lock"),
    (0x85, "KP ,", "Keypad comma"),
    (0x86, "KP =", "Keypad equals (AS/400)"),
    (0x87, "Ro", "International 1 (Ro)"),
    (0x88, "Kana", "International 2 (Katakana/Hiragana)"),
    (0x89, "¥", "International 3 (Yen)"),
    (0x8A, "Henk", "International 4 (Henkan)"),
    (0x8B, "Muhen", "International 5 (Muhenkan)"),
    (0x8C, "Int6", "International 6"),
    (0x8D, "Int7", "International 7"),
    (0x8E, "Int8", "International 8"),
    (0x8F, "Int9", "International 9"),
    (0x90, "Lang1", "Language 1 (Hangul/English)"),
    (0x91, "Lang2", "Language 2 (Hanja)"),
    (0x92, "Lang3", "Language 3 (Katakana)"),
    (0x93, "Lang4", "Language 4 (Hiragana)"),
    (0x94, "Lang5", "Language 5 (Zenkaku/Hankaku)"),
    (0x95, "Lang6", "Language 6"),
    (0x96, "Lang7", "Language 7"),
    (0x97, "Lang8", "Language 8"),
    (0x98, "Lang9", "Language 9"),
    (0x99, "Erase", "Alternate erase"),
    (0x9A, "SysRq", "SysReq/Attention"),
    (0x9B, "Cancel", "Cancel"),
    (0x9D, "Prior", "Prior"),
    (0x9E, "Return", "Return"),
    (0x9F, "Sep", "Separator"),
    (0xA0, "Out", "Out"),
    (0xA1, "Oper", "Oper"),
    (0xA2, "Clear", "Clear/Again"),
    (0xA3, "CrSel", "CrSel/Props"),
    (0xA4, "ExSel", "ExSel"),
    (0xB0, "KP 00", "Keypad 00"),
    (0xB1, "KP 000", "Keypad 000"),
    (0xB2, "Th sep", "Thousands separator"),
    (0xB3, "Dec sep", "Decimal separator"),
    (0xB4, "Cur", "Currency unit"),
    (0xB5, "Cur sub", "Currency sub-unit"),
    (0xB6, "KP (", "Keypad left parenthesis"),
    (0xB7, "KP )", "Keypad right parenthesis"),
    (0xB8, "KP {", "Keypad left brace"),
    (0xB9, "KP }", "Keypad right brace"),
    (0xBA, "KP Tab", "Keypad Tab"),
    (0xBB, "KP Bksp", "Keypad Backspace"),
    (0xBC, "KP A", "Keypad A"),
    (0xBD, "KP B", "Keypad B"),
    (0xBE, "KP C", "Keypad C"),
    (0xBF, "KP D", "Keypad D"),
    (0xC0, "KP E", "Keypad E"),
    (0xC1, "KP F", "Keypad F"),
    (0xC2, "KP XOR", "Keypad XOR"),
    (0xC3, "KP ^", "Keypad caret"),
    (0xC4, "KP %", "Keypad percent"),
    (0xC5, "KP <", "Keypad less than"),
    (0xC6, "KP >", "Keypad greater than"),
    (0xC7, "KP &", "Keypad ampersand"),
    (0xC8, "KP &&", "Keypad double ampersand"),
    (0xC9, "KP |", "Keypad bar"),
    (0xCA, "KP ||", "Keypad double bar"),
    (0xCB, "KP :", "Keypad colon"),
    (0xCC, "KP #", "Keypad hash"),
    (0xCD, "KP Spc", "Keypad Space"),
    (0xCE, "KP @", "Keypad at"),
    (0xCF, "KP !", "Keypad exclamation mark"),
    (0xD0, "KP MS", "Keypad memory store"),
    (0xD1, "KP MR", "Keypad memory recall"),
    (0xD2, "KP MC", "Keypad memory clear"),
    (0xD3, "KP M+", "Keypad memory add"),
    (0xD4, "KP M-", "Keypad memory subtract"),
    (0xD5, "KP M*", "Keypad memory multiply"),
    (0xD6, "KP M/", "Keypad memory divide"),
    (0xD7, "KP ±", "Keypad plus/minus"),
    (0xD8, "KP Clr", "Keypad clear"),
    (0xD9, "KP CE", "Keypad clear entry"),
    (0xDA, "KP Bin", "Keypad binary"),
    (0xDB, "KP Oct", "Keypad octal"),
    (0xDC, "KP Dec", "Keypad decimal"),
    (0xDD, "KP Hex", "Keypad hexadecimal"),
    (0xE0, "Ctrl", "Left Control"),
    (0xE1, "Shift", "Left Shift"),
    (0xE2, "Alt", "Left Alt"),
    (0xE3, "Win", "Left GUI (Windows/Command)"),
    (0xE4, "RCtrl", "Right Control"),
    (0xE5, "RShift", "Right Shift"),
    (0xE6, "RAlt", "Right Alt"),
    (0xE7, "RWin", "Right GUI (Windows/Command)"),
];

pub(crate) const CONSUMER: &[(u16, &str, &str)] = &[
    (0x00B5, "Next", "Next track"),
    (0x00B6, "Prev", "Previous track"),
    (0x00B7, "Stop", "Stop media"),
    (0x00CD, "Play", "Play/Pause"),
    (0x00E2, "Mute", "Mute"),
    (0x00E9, "Vol +", "Volume up"),
    (0x00EA, "Vol -", "Volume down"),
    (0x006F, "Bri +", "Display brightness up"),
    (0x0070, "Bri -", "Display brightness down"),
    (0x0183, "Media", "Open media player"),
    (0x018A, "Mail", "Open email client"),
    (0x0192, "Calc", "Open calculator"),
    (0x0194, "Files", "Open file browser"),
    (0x0221, "Search", "Search"),
    (0x0223, "Web", "Open browser home"),
    (0x0224, "Back", "Browser back"),
    (0x0225, "Fwd", "Browser forward"),
    (0x0227, "Reload", "Browser refresh"),
    (0x022A, "Bookm", "Browser bookmarks"),
    (0x029F, "Mission", "Mission Control"),
    (0x02A0, "Launch", "Launchpad"),
];

pub(crate) const SYSTEM: &[(SystemControl, &str, &str)] = &[
    (SystemControl::PowerDown, "Power", "System power down"),
    (SystemControl::Sleep, "Sleep", "System sleep"),
    (SystemControl::WakeUp, "Wake", "System wake up"),
];

pub(crate) const MOUSE: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "M1", "Left mouse button"),
    (MouseButton::Right, "M2", "Right mouse button"),
    (MouseButton::Middle, "M3", "Middle mouse button"),
    (MouseButton::Back, "M4", "Mouse back"),
    (MouseButton::Forward, "M5", "Mouse forward"),
];

pub(crate) const GAMEPAD: &[(Control, &str, &str)] = &[
    (Control::ButtonA, "A", "Button A"),
    (Control::ButtonB, "B", "Button B"),
    (Control::ButtonX, "X", "Button X"),
    (Control::ButtonY, "Y", "Button Y"),
    (Control::Start, "Start", "Start"),
    (Control::Back, "Back", "Back/Select"),
    (Control::Guide, "Guide", "Guide/Menu"),
    (Control::LeftBumper, "LB", "Left bumper"),
    (Control::RightBumper, "RB", "Right bumper"),
    (Control::LeftTrigger, "LT", "Left trigger"),
    (Control::RightTrigger, "RT", "Right trigger"),
    (Control::DpadUp, "D↑", "D-pad up"),
    (Control::DpadDown, "D↓", "D-pad down"),
    (Control::DpadLeft, "D←", "D-pad left"),
    (Control::DpadRight, "D→", "D-pad right"),
    (Control::LeftStickUp, "LS↑", "Left stick up"),
    (Control::LeftStickDown, "LS↓", "Left stick down"),
    (Control::LeftStickLeft, "LS←", "Left stick left"),
    (Control::LeftStickRight, "LS→", "Left stick right"),
    (Control::LeftStickPress, "LS", "Left stick press"),
    (Control::RightStickUp, "RS↑", "Right stick up"),
    (Control::RightStickDown, "RS↓", "Right stick down"),
    (Control::RightStickLeft, "RS←", "Right stick left"),
    (Control::RightStickRight, "RS→", "Right stick right"),
    (Control::RightStickPress, "RS", "Right stick press"),
    (Control::Accelerator, "Accel", "Accelerator"),
    (Control::Rudder, "Rudder", "Rudder"),
    (Control::Trigger, "Trig", "Trigger axis"),
    (Control::RxNegative, "Rx-", "X rotation negative"),
    (Control::RxPositive, "Rx+", "X rotation positive"),
    (Control::RyNegative, "Ry-", "Y rotation negative"),
    (Control::RyPositive, "Ry+", "Y rotation positive"),
    (Control::HatUp, "Hat↑", "Hat up"),
    (Control::HatDown, "Hat↓", "Hat down"),
    (Control::HatLeft, "Hat←", "Hat left"),
    (Control::HatRight, "Hat→", "Hat right"),
];

pub(crate) fn legend(action: Action) -> Cow<'static, str> {
    describe(action, Length::Short)
}

pub(crate) fn name(action: Action) -> Cow<'static, str> {
    describe(action, Length::Long)
}

pub(crate) const fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Base layer",
        Layer::One => "Fn layer 1",
        Layer::Two => "Fn layer 2",
        Layer::Three => "Fn layer 3",
    }
}

pub(crate) fn control(control: Control) -> (&'static str, &'static str) {
    GAMEPAD
        .iter()
        .find(|entry| entry.0 == control)
        .map_or(("?", "Unknown control"), |entry| (entry.1, entry.2))
}

#[derive(Clone, Copy)]
enum Length {
    Short,
    Long,
}

impl Length {
    const fn pick(self, short: &'static str, long: &'static str) -> &'static str {
        match self {
            Self::Short => short,
            Self::Long => long,
        }
    }
}

fn describe(action: Action, length: Length) -> Cow<'static, str> {
    let named = |names: Option<(&'static str, &'static str)>, fallback: String| {
        names.map_or(Cow::Owned(fallback), |(short, long)| {
            Cow::Borrowed(length.pick(short, long))
        })
    };
    match action {
        Action::None => Cow::Borrowed(length.pick("", "Nothing")),
        Action::Keyboard(usage) => named(
            KEYBOARD
                .iter()
                .find(|entry| entry.0 == usage)
                .map(|entry| (entry.1, entry.2)),
            format!("0x{usage:02x}"),
        ),
        Action::Consumer(usage) => named(
            CONSUMER
                .iter()
                .find(|entry| entry.0 == usage)
                .map(|entry| (entry.1, entry.2)),
            format!("C 0x{usage:04x}"),
        ),
        Action::System(system) => named(
            SYSTEM
                .iter()
                .find(|entry| entry.0 == system)
                .map(|entry| (entry.1, entry.2)),
            String::new(),
        ),
        Action::Mouse(button) => named(
            MOUSE
                .iter()
                .find(|entry| entry.0 == button)
                .map(|entry| (entry.1, entry.2)),
            String::new(),
        ),
        Action::Gamepad(gamepad) => {
            let (short, long) = control(gamepad);
            Cow::Borrowed(length.pick(short, long))
        }
        Action::Layer(layer) => Cow::Borrowed(match layer {
            Layer::Base => length.pick("Base", "Base layer"),
            Layer::One => length.pick("Fn", "Fn layer 1 (momentary)"),
            Layer::Two => length.pick("Fn2", "Fn layer 2 (momentary)"),
            Layer::Three => length.pick("Fn3", "Fn layer 3 (momentary)"),
        }),
        Action::Function(Function::Profile(slot)) => {
            let number = u16::from(slot.get()) + 1;
            Cow::Owned(match length {
                Length::Short => format!("P{number}"),
                Length::Long => format!("Switch to profile {number}"),
            })
        }
        Action::Function(function) => Cow::Borrowed(match function {
            Function::BrightnessUp => length.pick("Bri+", "Keyboard brightness up"),
            Function::BrightnessDown => length.pick("Bri-", "Keyboard brightness down"),
            Function::Profile(_) | Function::NextProfile => length.pick("Next P", "Next profile"),
            Function::ToggleModeProfile => length.pick(
                "Mode",
                "Mode key: toggle between the first and the last used profile",
            ),
            Function::ToggleKeyDisable => length.pick("Lock", "Toggle key disable"),
            Function::LayerLock => length.pick("FnLk", "Fn layer lock"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_keyboard_usage_the_firmware_accepts_has_a_name() {
        for usage in Action::KEYBOARD_USAGES.into_iter().flatten() {
            assert!(
                KEYBOARD.iter().any(|entry| entry.0 == usage),
                "usage {usage:#04x} has no name"
            );
        }
    }

    #[test]
    fn profile_functions_name_their_profile_from_one() {
        let second = Action::Function(Function::Profile(openwoot_protocol::profile::Slot::new(1)));
        assert_eq!(legend(second), "P2");
        assert_eq!(name(second), "Switch to profile 2");
    }
}
