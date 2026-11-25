use openwoot_protocol::keys::{Action, Keymap};

use super::quick_bind::usage_of_character;

const LEET: [(char, char); 10] = [
    ('a', '4'),
    ('b', '8'),
    ('e', '3'),
    ('g', '6'),
    ('i', '1'),
    ('l', '1'),
    ('o', '0'),
    ('s', '5'),
    ('t', '7'),
    ('z', '2'),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Factory {
    Default,
    Function,
    Leet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Defaults {
    pub(crate) main: Keymap,
    pub(crate) function: Keymap,
}

impl Factory {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Function => "Function",
            Self::Leet => "1337",
        }
    }

    pub(crate) const fn sample(self) -> &'static str {
        match self {
            Self::Default => "QWERTY",
            Self::Function => "Fn",
            Self::Leet => "QW3R7",
        }
    }

    pub(crate) fn keymap(self, defaults: &Defaults) -> Keymap {
        match self {
            Self::Default => defaults.main.clone(),
            Self::Function => defaults.function.clone(),
            Self::Leet => leet(&defaults.main),
        }
    }
}

fn leet(keymap: &Keymap) -> Keymap {
    let mut converted = keymap.clone();
    for action in &mut converted {
        if let Action::Keyboard(code) = *action
            && let Some(digit) = LEET
                .iter()
                .find(|(letter, _)| usage_of_character(*letter) == Some(code))
                .and_then(|(_, digit)| usage_of_character(*digit))
        {
            *action = Action::Keyboard(digit);
        }
    }
    converted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> Keymap {
        text.chars()
            .map(|character| usage_of_character(character).map_or(Action::None, Action::Keyboard))
            .collect()
    }

    #[test]
    fn leet_swaps_letters_for_digits_and_keeps_the_rest() {
        let defaults = Defaults {
            main: typed("leet speak, 2024"),
            function: typed("fn"),
        };
        assert_eq!(Factory::Leet.keymap(&defaults), typed("1337 5p34k, 2024"));
        assert_eq!(Factory::Default.keymap(&defaults), defaults.main);
        assert_eq!(Factory::Function.keymap(&defaults), defaults.function);
    }
}
