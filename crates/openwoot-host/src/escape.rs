#[must_use]
pub fn control_characters(text: &str) -> String {
    text.chars().fold(String::new(), |mut shown, character| {
        if character.is_control() {
            shown.extend(character.escape_default());
        } else {
            shown.push(character);
        }
        shown
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_from_a_device_or_a_file_are_shown_escaped() {
        assert_eq!(
            control_characters("Test\u{1b}[2J board\nboard: forged\r\u{7f}\u{85}"),
            "Test\\u{1b}[2J board\\nboard: forged\\r\\u{7f}\\u{85}"
        );
    }

    #[test]
    fn printable_text_is_kept_as_it_is() {
        let text = "Wooting Restore (80HE) \"quoted\" \\ ü";
        assert_eq!(control_characters(text), text);
    }
}
