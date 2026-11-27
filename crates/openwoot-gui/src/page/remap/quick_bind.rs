use gpui_kit::Modifiers;

pub(crate) const CAPS_LOCK: u8 = 0x39;

const LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "1234567890";
const SHIFTED_DIGITS: &str = "!@#$%^&*()";
const FIRST_LETTER: u8 = 0x04;
const FIRST_DIGIT: u8 = 0x1E;
const FIRST_FUNCTION: u8 = 0x3A;
const FIRST_HIGH_FUNCTION: u8 = 0x68;

pub(crate) fn usage_of_key(key: &str) -> Option<u8> {
    let named = match key {
        "enter" => 0x28,
        "escape" => 0x29,
        "backspace" => 0x2A,
        "tab" => 0x2B,
        "space" | " " => 0x2C,
        "-" | "_" => 0x2D,
        "=" | "+" => 0x2E,
        "[" | "{" => 0x2F,
        "]" | "}" => 0x30,
        "\\" | "|" => 0x31,
        ";" | ":" => 0x33,
        "'" | "\"" => 0x34,
        "`" | "~" => 0x35,
        "," | "<" => 0x36,
        "." | ">" => 0x37,
        "/" | "?" => 0x38,
        "insert" => 0x49,
        "home" => 0x4A,
        "pageup" => 0x4B,
        "delete" => 0x4C,
        "end" => 0x4D,
        "pagedown" => 0x4E,
        "right" => 0x4F,
        "left" => 0x50,
        "down" => 0x51,
        "up" => 0x52,
        _ => return character_usage(key).or_else(|| function_usage(key)),
    };
    Some(named)
}

pub(crate) fn usage_of_character(character: char) -> Option<u8> {
    usage_of_key(character.encode_utf8(&mut [0; 4]))
}

pub(crate) fn pressed_modifier(before: Modifiers, after: Modifiers) -> Option<u8> {
    let held: Vec<u8> = modifier_usages(before).collect();
    modifier_usages(after).find(|usage| !held.contains(usage))
}

pub(crate) fn modifier_usages(modifiers: Modifiers) -> impl Iterator<Item = u8> {
    [
        (modifiers.control, 0xE0),
        (modifiers.shift, 0xE1),
        (modifiers.alt, 0xE2),
        (modifiers.platform, 0xE3),
    ]
    .into_iter()
    .filter_map(|(held, usage)| held.then_some(usage))
}

fn character_usage(key: &str) -> Option<u8> {
    let mut characters = key.chars();
    let (Some(character), None) = (characters.next(), characters.next()) else {
        return None;
    };
    let lower = character.to_ascii_lowercase();
    offset_in(LETTERS, lower)
        .map(|offset| FIRST_LETTER + offset)
        .or_else(|| offset_in(DIGITS, lower).map(|offset| FIRST_DIGIT + offset))
        .or_else(|| offset_in(SHIFTED_DIGITS, lower).map(|offset| FIRST_DIGIT + offset))
}

fn function_usage(key: &str) -> Option<u8> {
    let number: u8 = key.strip_prefix('f')?.parse().ok()?;
    match number {
        1..=12 => Some(FIRST_FUNCTION + number - 1),
        13..=24 => Some(FIRST_HIGH_FUNCTION + number - 13),
        _ => None,
    }
}

fn offset_in(characters: &str, character: char) -> Option<u8> {
    characters
        .find(character)
        .and_then(|offset| u8::try_from(offset).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_map_to_their_report_usages() {
        assert_eq!(usage_of_key("a"), Some(0x04));
        assert_eq!(usage_of_key("Z"), Some(0x1D));
        assert_eq!(usage_of_key("1"), Some(0x1E));
        assert_eq!(usage_of_key("9"), Some(0x26));
        assert_eq!(usage_of_key("0"), Some(0x27));
        assert_eq!(usage_of_key("!"), Some(0x1E));
        assert_eq!(usage_of_key(")"), Some(0x27));
    }

    #[test]
    fn named_keys_and_function_keys_map_to_their_report_usages() {
        assert_eq!(usage_of_key("enter"), Some(0x28));
        assert_eq!(usage_of_key("escape"), Some(0x29));
        assert_eq!(usage_of_key("space"), Some(0x2C));
        assert_eq!(usage_of_key("delete"), Some(0x4C));
        assert_eq!(usage_of_key("up"), Some(0x52));
        assert_eq!(usage_of_key("f1"), Some(0x3A));
        assert_eq!(usage_of_key("f12"), Some(0x45));
        assert_eq!(usage_of_key("f13"), Some(0x68));
        assert_eq!(usage_of_key("f24"), Some(0x73));
    }

    #[test]
    fn punctuation_maps_with_and_without_shift() {
        assert_eq!(usage_of_key(";"), Some(0x33));
        assert_eq!(usage_of_key(":"), Some(0x33));
        assert_eq!(usage_of_key("/"), Some(0x38));
        assert_eq!(usage_of_key("?"), Some(0x38));
        assert_eq!(usage_of_character('\''), Some(0x34));
        assert_eq!(usage_of_character('-'), Some(0x2D));
    }

    #[test]
    fn keys_without_a_clear_usage_are_not_mapped() {
        assert_eq!(usage_of_key("f25"), None);
        assert_eq!(usage_of_key("é"), None);
        assert_eq!(usage_of_key("back"), None);
        assert_eq!(usage_of_key(""), None);
    }

    #[test]
    fn a_newly_held_modifier_maps_to_its_left_usage() {
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let shift_and_command = Modifiers {
            platform: true,
            ..shift
        };
        assert_eq!(pressed_modifier(none, shift), Some(0xE1));
        assert_eq!(pressed_modifier(shift, shift_and_command), Some(0xE3));
        assert_eq!(pressed_modifier(shift, none), None);
    }

    #[test]
    fn held_modifiers_map_to_their_left_usages_in_report_order() {
        let held = Modifiers {
            control: true,
            alt: true,
            platform: true,
            ..Modifiers::default()
        };
        assert_eq!(
            modifier_usages(held).collect::<Vec<_>>(),
            [0xE0, 0xE2, 0xE3]
        );
        assert_eq!(modifier_usages(Modifiers::default()).count(), 0);
    }
}
