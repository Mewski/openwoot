use openwoot_protocol::macros::{TEXT_LEN, Text, is_typable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Problem {
    Untypable(char),
    TooLong,
}

impl Problem {
    pub(super) fn message(self) -> String {
        match self {
            Self::Untypable(character) => {
                format!("“{character}” can't be typed, use plain ASCII text")
            }
            Self::TooLong => format!("A text step holds up to {TEXT_LEN} characters"),
        }
    }
}

pub(super) fn shown(text: &str) -> String {
    let mut shown = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' => shown.push_str("\\n"),
            '\t' => shown.push_str("\\t"),
            '\\' => shown.push_str("\\\\"),
            character => shown.push(character),
        }
    }
    shown
}

pub(super) fn parsed(typed: &str) -> Result<Text, Problem> {
    let mut text = String::with_capacity(typed.len());
    let mut characters = typed.chars();
    while let Some(character) = characters.next() {
        let character = match character {
            '\\' => match characters.next() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('\\') | None => '\\',
                Some(other) => {
                    text.push('\\');
                    other
                }
            },
            character => character,
        };
        if !is_typable(character) {
            return Err(Problem::Untypable(character));
        }
        text.push(character);
    }
    Text::try_from(text.as_str()).map_err(|_| Problem::TooLong)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_lines_tabs_and_backslashes_are_shown_escaped_and_read_back() {
        let text = "Hi,\n\tC:\\temp";
        let typed = shown(text);
        assert_eq!(typed, "Hi,\\n\\tC:\\\\temp");
        assert_eq!(parsed(&typed).unwrap().as_str(), text);
    }

    #[test]
    fn an_unknown_escape_or_a_trailing_backslash_stays_literal() {
        assert_eq!(parsed("a\\b").unwrap().as_str(), "a\\b");
        assert_eq!(parsed("end\\").unwrap().as_str(), "end\\");
    }

    #[test]
    fn only_typable_text_of_up_to_sixty_four_characters_is_accepted() {
        assert_eq!(parsed("née"), Err(Problem::Untypable('é')));
        assert_eq!(parsed(&"a".repeat(TEXT_LEN + 1)), Err(Problem::TooLong));
        assert_eq!(
            parsed(&"a".repeat(TEXT_LEN)).map(|text| text.len()),
            Ok(TEXT_LEN)
        );
        assert_eq!(parsed("").map(|text| text.len()), Ok(0));
    }
}
