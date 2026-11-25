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
    (0x1E, "1", "1"),
    (0x1F, "2", "2"),
    (0x20, "3", "3"),
    (0x21, "4", "4"),
    (0x22, "5", "5"),
    (0x23, "6", "6"),
    (0x24, "7", "7"),
    (0x25, "8", "8"),
    (0x26, "9", "9"),
    (0x27, "0", "0"),
    (0x28, "Enter", "Enter"),
    (0x29, "Esc", "Escape"),
    (0x2A, "Backspace", "Backspace"),
    (0x2B, "Tab", "Tab"),
    (0x2C, "Spacebar", "Space"),
    (0x2D, "- _", "Underscore"),
    (0x2E, "= +", "Plus"),
    (0x2F, "[ {", "OpenBracket"),
    (0x30, "] }", "CloseBracket"),
    (0x31, "\\", "Backslash"),
    (0x33, "; :", "Colon"),
    (0x34, "'\"", "Quote"),
    (0x35, "` ~", "Tilde"),
    (0x36, ", <", "Comma"),
    (0x37, ". >", "Dot"),
    (0x38, "/ ?", "Slash"),
    (0x39, "Caps", "CapsLock"),
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
    (0x46, "Prt Sc", "Print Screen"),
    (0x47, "Scr Lk", "Scroll Lock"),
    (0x48, "Pause", "Pause / Break"),
    (0x49, "Ins", "Insert"),
    (0x4A, "Home", "Home"),
    (0x4B, "Pg Up", "Page Up"),
    (0x4C, "Del", "Delete"),
    (0x4D, "End", "End"),
    (0x4E, "Pg Dn", "Page Down"),
    (0x4F, "→", "Right Arrow"),
    (0x50, "←", "Left Arrow"),
    (0x51, "↓", "Down Arrow"),
    (0x52, "↑", "Up Arrow"),
    (0x53, "Num Lock", "Numpad Lock"),
    (0x54, "Num /", "Numpad Divide"),
    (0x55, "Num *", "Numpad Multiply"),
    (0x56, "Num -", "Numpad Minus"),
    (0x57, "Num +", "Numpad Plus"),
    (0x58, "Enter", "Numpad Enter"),
    (0x59, "Num 1", "Numpad 1"),
    (0x5A, "Num 2", "Numpad 2"),
    (0x5B, "Num 3", "Numpad 3"),
    (0x5C, "Num 4", "Numpad 4"),
    (0x5D, "Num 5", "Numpad 5"),
    (0x5E, "Num 6", "Numpad 6"),
    (0x5F, "Num 7", "Numpad 7"),
    (0x60, "Num 8", "Numpad 8"),
    (0x61, "Num 9", "Numpad 9"),
    (0x62, "Num 0", "Numpad 0"),
    (0x63, "Num .", "Numpad Decimal"),
    (0x64, "\\", "ISO Backslash"),
    (0x65, "Menu", "Menu"),
    (0x66, "Power", "Power"),
    (0x67, "Num =", "Numpad Equals"),
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
    (0x76, "Menu", "Keyboard Menu"),
    (0x77, "Select", "Select"),
    (0x78, "Stop", "Stop"),
    (0x79, "Again", "Again"),
    (0x7A, "Undo", "Undo"),
    (0x7B, "Cut", "Cut"),
    (0x7C, "Copy", "Copy"),
    (0x7D, "Paste", "Paste"),
    (0x7E, "Find", "Find"),
    (0x7F, "Mute", "Keyboard Mute"),
    (0x80, "Vol +", "Keyboard Volume Up"),
    (0x81, "Vol -", "Keyboard Volume Down"),
    (0x82, "L-Caps", "Locking Caps Lock"),
    (0x83, "L-Num", "Locking Num Lock"),
    (0x84, "L-Scr", "Locking Scroll Lock"),
    (0x85, "Num ,", "Numpad Comma"),
    (0x86, "Num =", "Numpad Equals (AS/400)"),
    (0x87, "ろ/Ro", "Ro [HID Int1]"),
    (0x88, "Kana", "Katakana/Hiragana [HID Int2]"),
    (0x89, "¥/Yen", "Yen [HID Int3]"),
    (0x8A, "Conv", "Henkan [HID Int4]"),
    (0x8B, "Non-conv", "Muhenkan [HID Int5]"),
    (0x8C, "Int 6", "... [HID Int6]"),
    (0x8D, "Int 7", "... [HID Int7]"),
    (0x8E, "Int 8", "... [HID Int8]"),
    (0x8F, "Int 9", "... [HID Int9]"),
    (0x90, "Ime On", "Ime On [HID Lang1]"),
    (0x91, "Ime Off", "Ime Off [HID Lang2]"),
    (0x92, "Lang 3", "Katakana [HID Lang3]"),
    (0x93, "Lang 4", "Hiragana [HID Lang4]"),
    (0x94, "Lang 5", "... [HID Lang5]"),
    (0x95, "Lang 6", "... [HID Lang6]"),
    (0x96, "Lang 7", "... [HID Lang7]"),
    (0x97, "Lang 8", "... [HID Lang8]"),
    (0x98, "Lang 9", "... [HID Lang9]"),
    (0x99, "Erase", "Alternate Erase"),
    (0x9A, "SysRq", "SysReq / Attention"),
    (0x9B, "Cancel", "Cancel"),
    (0x9D, "Prior", "Prior"),
    (0x9E, "Return", "Return"),
    (0x9F, "Sep", "Separator"),
    (0xA0, "Out", "Out"),
    (0xA1, "Oper", "Oper"),
    (0xA2, "Clear", "Clear / Again"),
    (0xA3, "CrSel", "CrSel / Props"),
    (0xA4, "ExSel", "ExSel"),
    (0xB0, "Num 00", "Numpad 00"),
    (0xB1, "Num 000", "Numpad 000"),
    (0xB2, "Th Sep", "Thousands Separator"),
    (0xB3, "Dec Sep", "Decimal Separator"),
    (0xB4, "Cur", "Currency Unit"),
    (0xB5, "Cur Sub", "Currency Sub-unit"),
    (0xB6, "Num (", "Numpad Left Parenthesis"),
    (0xB7, "Num )", "Numpad Right Parenthesis"),
    (0xB8, "Num {", "Numpad Left Brace"),
    (0xB9, "Num }", "Numpad Right Brace"),
    (0xBA, "Num Tab", "Numpad Tab"),
    (0xBB, "Num Bksp", "Numpad Backspace"),
    (0xBC, "Num A", "Numpad A"),
    (0xBD, "Num B", "Numpad B"),
    (0xBE, "Num C", "Numpad C"),
    (0xBF, "Num D", "Numpad D"),
    (0xC0, "Num E", "Numpad E"),
    (0xC1, "Num F", "Numpad F"),
    (0xC2, "Num XOR", "Numpad XOR"),
    (0xC3, "Num ^", "Numpad Caret"),
    (0xC4, "Num %", "Numpad Percent"),
    (0xC5, "Num <", "Numpad Less Than"),
    (0xC6, "Num >", "Numpad Greater Than"),
    (0xC7, "Num &", "Numpad Ampersand"),
    (0xC8, "Num &&", "Numpad Double Ampersand"),
    (0xC9, "Num |", "Numpad Bar"),
    (0xCA, "Num ||", "Numpad Double Bar"),
    (0xCB, "Num :", "Numpad Colon"),
    (0xCC, "Num #", "Numpad Hash"),
    (0xCD, "Num Space", "Numpad Space"),
    (0xCE, "Num @", "Numpad At"),
    (0xCF, "Num !", "Numpad Exclamation Mark"),
    (0xD0, "Num MS", "Numpad Memory Store"),
    (0xD1, "Num MR", "Numpad Memory Recall"),
    (0xD2, "Num MC", "Numpad Memory Clear"),
    (0xD3, "Num M+", "Numpad Memory Add"),
    (0xD4, "Num M-", "Numpad Memory Subtract"),
    (0xD5, "Num M*", "Numpad Memory Multiply"),
    (0xD6, "Num M/", "Numpad Memory Divide"),
    (0xD7, "Num ±", "Numpad Plus / Minus"),
    (0xD8, "Num Clr", "Numpad Clear"),
    (0xD9, "Num CE", "Numpad Clear Entry"),
    (0xDA, "Num Bin", "Numpad Binary (Base 2)"),
    (0xDB, "Num Oct", "Numpad Octal (Base 8)"),
    (0xDC, "Num Dec", "Numpad Decimal (Base 10)"),
    (0xDD, "Num Hex", "Numpad Hexadecimal (Base 16)"),
    (0xE0, "L-Ctrl", "Control (Left)"),
    (0xE1, "L-Shift", "Shift (Left)"),
    (0xE2, "L-Alt", "Alt / ⌥ Option (Left)"),
    (0xE3, "L-Win", "Windows / Super / ⌘ Command (Left)"),
    (0xE4, "R-Ctrl", "Control (Right)"),
    (0xE5, "R-Shift", "Shift (Right)"),
    (0xE6, "R-Alt", "Alt / ⌥ Option (Right)"),
    (0xE7, "R-Win", "Windows / Super / ⌘ Command (Right)"),
];

pub(crate) const CONSUMER: &[(u16, &str, &str)] = &[
    (0x00B5, "Next Track", "Next Track"),
    (0x00B6, "Previous Track", "Previous Track"),
    (0x00B7, "Stop", "Stop Media"),
    (0x00CD, "Play / Pause", "Play / Pause"),
    (0x00E2, "Volume Mute", "Mute Volume"),
    (0x00E9, "Volume Up", "Increase Volume"),
    (0x00EA, "Volume Down", "Decrease Volume"),
    (0x006F, "Display Brightness Up", "Display Brightness Up"),
    (0x0070, "Display Brightness Down", "Display Brightness Down"),
    (0x0183, "Music", "Open Mediaplayer"),
    (0x018A, "Email", "Open Email Client"),
    (0x0192, "Calculator", "Open Calculator"),
    (0x0194, "Files", "Open Filebrowser"),
    (0x0221, "Search", "Search"),
    (0x0223, "Browser & Home", "Open Browser"),
    (0x0224, "Back", "Back Browser"),
    (0x0225, "Forward", "Forward Browser"),
    (0x0227, "Refresh", "Refresh Browser"),
    (0x022A, "Bookmarks", "Bookmark Browser"),
    (
        0x029F,
        "Mission Control",
        "MacOS Mission Control (Show All Windows)",
    ),
    (
        0x02A0,
        "Launchpad",
        "MacOS Launchpad (Application Launcher)",
    ),
];

pub(crate) const SYSTEM: &[(SystemControl, &str, &str)] = &[
    (SystemControl::PowerDown, "Power Down", "System Power"),
    (SystemControl::Sleep, "Sleep", "System Sleep"),
    (SystemControl::WakeUp, "Wake Up", "System Wake Up"),
];

pub(crate) const MOUSE: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Left, "Mouse L", "Left Mouse button (Mouse 1)"),
    (
        MouseButton::Right,
        "Mouse R",
        "Right Mouse button (Mouse 2)",
    ),
    (
        MouseButton::Middle,
        "Mouse M",
        "Middle Mouse button (Mouse 3)",
    ),
    (
        MouseButton::Back,
        "Mouse 4",
        "Commonly used as the back button on the side of mice (Mouse 4)",
    ),
    (
        MouseButton::Forward,
        "Mouse 5",
        "Commonly used as the forward button on the side of mice (Mouse 5)",
    ),
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

const PROFILES: [&str; 4] = [
    "Switch to the First Profile",
    "Switch To the Second Profile",
    "Switch To the Third Profile",
    "Switch To the Fourth Profile",
];

const MAC_LEGENDS: &[(u8, &str)] = &[
    (0xE0, "^ Ctrl"),
    (0xE2, "⌥ Opt"),
    (0xE3, "⌘ Cmd"),
    (0xE4, "^ Ctrl"),
    (0xE6, "⌥ Opt"),
    (0xE7, "⌘ Cmd"),
];

const LINUX_LEGENDS: &[(u8, &str)] = &[(0xE3, "L-Super"), (0xE7, "R-Super")];

pub(crate) fn legend(action: Action) -> Cow<'static, str> {
    describe(action, Length::Short)
}

pub(crate) fn name(action: Action) -> Cow<'static, str> {
    describe(action, Length::Long)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Mac,
    Linux,
    Other,
}

impl Length {
    const fn pick(self, short: &'static str, long: &'static str) -> &'static str {
        match self {
            Self::Short => short,
            Self::Long => long,
        }
    }
}

impl Platform {
    const CURRENT: Self = if cfg!(target_os = "macos") {
        Self::Mac
    } else if cfg!(target_os = "linux") {
        Self::Linux
    } else {
        Self::Other
    };

    fn legend(self, usage: u8) -> Option<&'static str> {
        let legends = match self {
            Self::Mac => MAC_LEGENDS,
            Self::Linux => LINUX_LEGENDS,
            Self::Other => &[],
        };
        legends
            .iter()
            .find(|entry| entry.0 == usage)
            .map(|entry| entry.1)
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
        Action::Keyboard(usage) => match (length, Platform::CURRENT.legend(usage)) {
            (Length::Short, Some(legend)) => Cow::Borrowed(legend),
            _ => named(
                KEYBOARD
                    .iter()
                    .find(|entry| entry.0 == usage)
                    .map(|entry| (entry.1, entry.2)),
                format!("0x{usage:02x}"),
            ),
        },
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
            Layer::One => length.pick(
                "Fn 1",
                "Fn layer 1. While pressed, access Fn layer 1 mapped keys",
            ),
            Layer::Two => length.pick(
                "Fn 2",
                "Fn layer 2. While pressed, access Fn layer 2 mapped keys",
            ),
            Layer::Three => length.pick(
                "Fn 3",
                "Fn layer 3. While pressed, access Fn layer 3 mapped keys",
            ),
        }),
        Action::Function(Function::Profile(slot)) => {
            let number = u16::from(slot.get()) + 1;
            match (length, PROFILES.get(usize::from(slot.get()))) {
                (Length::Short, _) => Cow::Owned(format!("P{number}")),
                (Length::Long, Some(name)) => Cow::Borrowed(name),
                (Length::Long, None) => Cow::Owned(format!("Switch to Profile {number}")),
            }
        }
        Action::Function(function) => Cow::Borrowed(match function {
            Function::BrightnessUp => length.pick("Brightness Up", "Increase Keyboard Brightness"),
            Function::BrightnessDown => {
                length.pick("Brightness Down", "Decrease Keyboard Brightness")
            }
            Function::Profile(_) | Function::NextProfile => {
                length.pick("Cycle", "Cycle through all active profiles.")
            }
            Function::ToggleModeProfile => length.pick(
                "Mode",
                "Toggle between the default profile and the last other one.",
            ),
            Function::ToggleKeyDisable => length.pick(
                "Key Lock",
                "In the Fn layer, disables the same key on the main layer.",
            ),
            Function::LayerLock => {
                length.pick("Fn Lock", "Swaps the Fn layer with the main layer.")
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::profile::Slot;

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
    fn every_usage_is_named_once() {
        for (position, entry) in KEYBOARD.iter().enumerate() {
            assert!(
                KEYBOARD
                    .iter()
                    .skip(position + 1)
                    .all(|other| other.0 != entry.0),
                "usage {:#04x} is named twice",
                entry.0
            );
        }
        for (position, entry) in CONSUMER.iter().enumerate() {
            assert!(
                CONSUMER
                    .iter()
                    .skip(position + 1)
                    .all(|other| other.0 != entry.0),
                "consumer usage {:#06x} is named twice",
                entry.0
            );
        }
    }

    #[test]
    fn keys_read_like_wootility_keycaps() {
        let legend_of = |usage| legend(Action::Keyboard(usage));
        assert_eq!(legend_of(0x2A), "Backspace");
        assert_eq!(legend_of(0xE1), "L-Shift");
        assert_eq!(legend_of(0x46), "Prt Sc");
        assert_eq!(legend_of(0x47), "Scr Lk");
        assert_eq!(legend_of(0x5A), "Num 2");
        assert_eq!(legend_of(0x35), "` ~");
        assert_eq!(legend_of(0x29), "Esc");
        assert_eq!(legend_of(0x2C), "Spacebar");
        assert_eq!(legend_of(0x8B), "Non-conv");
        assert_eq!(legend(Action::Layer(Layer::One)), "Fn 1");
        assert_eq!(
            legend(Action::Function(Function::ToggleModeProfile)),
            "Mode"
        );
        assert_eq!(legend(Action::Consumer(0x00CD)), "Play / Pause");
        assert_eq!(legend(Action::Mouse(MouseButton::Left)), "Mouse L");
    }

    #[test]
    fn names_are_wootility_tooltips_or_key_ids() {
        let name_of = |usage| name(Action::Keyboard(usage));
        assert_eq!(name_of(0x46), "Print Screen");
        assert_eq!(name_of(0x5A), "Numpad 2");
        assert_eq!(name_of(0xE1), "Shift (Left)");
        assert_eq!(name_of(0x29), "Escape");
        assert_eq!(name_of(0x04), "A");
        assert_eq!(name(Action::Consumer(0x00E9)), "Increase Volume");
        assert_eq!(
            name(Action::Function(Function::BrightnessUp)),
            "Increase Keyboard Brightness"
        );
    }

    #[test]
    fn modifiers_read_like_the_platform_names_them() {
        assert_eq!(Platform::Mac.legend(0xE0), Some("^ Ctrl"));
        assert_eq!(Platform::Mac.legend(0xE2), Some("⌥ Opt"));
        assert_eq!(Platform::Mac.legend(0xE7), Some("⌘ Cmd"));
        assert_eq!(Platform::Mac.legend(0xE1), None);
        assert_eq!(Platform::Linux.legend(0xE3), Some("L-Super"));
        assert_eq!(Platform::Linux.legend(0xE0), None);
        assert_eq!(Platform::Other.legend(0xE3), None);
        let shown = legend(Action::Keyboard(0xE3));
        assert_eq!(
            shown,
            Platform::CURRENT.legend(0xE3).unwrap_or("L-Win"),
            "the current platform decides the legend"
        );
    }

    #[test]
    fn profile_functions_name_their_profile_from_one() {
        let second = Action::Function(Function::Profile(Slot::new(1)));
        assert_eq!(legend(second), "P2");
        assert_eq!(name(second), "Switch To the Second Profile");
        let sixth = Action::Function(Function::Profile(Slot::new(5)));
        assert_eq!(legend(sixth), "P6");
        assert_eq!(name(sixth), "Switch to Profile 6");
    }
}
