use openwoot_protocol::keys::{Action, Keymap};

use super::quick_bind::usage_of_character;

const QWERTY_KEYS: &str = "-=qwertyuiop[]asdfghjkl;'zxcvbnm,./";
const LETTER_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LetterLayout {
    Qwerty,
    Dvorak,
    Colemak,
    Workman,
    Alphabetical,
}

impl LetterLayout {
    pub(crate) const ALL: [Self; 5] = [
        Self::Qwerty,
        Self::Dvorak,
        Self::Colemak,
        Self::Workman,
        Self::Alphabetical,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Qwerty => "QWERTY",
            Self::Dvorak => "Dvorak",
            Self::Colemak => "Colemak",
            Self::Workman => "Workman",
            Self::Alphabetical => "ABC",
        }
    }

    const fn keys(self) -> &'static str {
        match self {
            Self::Qwerty => QWERTY_KEYS,
            Self::Dvorak => "[]',.pyfgcrl/=aoeuidhtns-;qjkxbmwvz",
            Self::Colemak => "-=qwfpgjluy;[]arstdhneio'zxcvbkm,./",
            Self::Workman => "-=qdrwbjfup;[]ashtgyneoi'zxmcvkl,./",
            Self::Alphabetical => "-=abcdefghij[]klmnopqrs;'tuvwxyz,./",
        }
    }

    fn typed_for(self, qwerty: u8) -> u8 {
        QWERTY_KEYS
            .chars()
            .zip(self.keys().chars())
            .find(|(key, _)| usage_of_character(*key) == Some(qwerty))
            .and_then(|(_, typed)| usage_of_character(typed))
            .unwrap_or(qwerty)
    }

    fn qwerty_for(self, typed: u8) -> u8 {
        QWERTY_KEYS
            .chars()
            .zip(self.keys().chars())
            .find(|(_, key)| usage_of_character(*key) == Some(typed))
            .and_then(|(qwerty, _)| usage_of_character(qwerty))
            .unwrap_or(typed)
    }
}

pub(crate) fn detect(
    keymap: &Keymap,
    place: impl Fn(usize) -> Option<(u16, u16)>,
) -> Option<LetterLayout> {
    LetterLayout::ALL
        .into_iter()
        .find(|layout| is_qwerty_through(keymap, *layout, &place))
}

pub(crate) fn convert(keymap: &Keymap, from: LetterLayout, to: LetterLayout) -> Keymap {
    let mut converted = keymap.clone();
    for action in &mut converted {
        if let Action::Keyboard(code) = *action {
            *action = Action::Keyboard(to.typed_for(from.qwerty_for(code)));
        }
    }
    converted
}

fn is_qwerty_through(
    keymap: &Keymap,
    layout: LetterLayout,
    place: &impl Fn(usize) -> Option<(u16, u16)>,
) -> bool {
    let mut row_above = None;
    for row in LETTER_ROWS {
        let mut row_height = None;
        let mut left = None;
        for letter in row.chars() {
            let Some(code) = usage_of_character(letter) else {
                return false;
            };
            let mut holders = keymap.iter().enumerate().filter_map(|(key, action)| {
                matches!(action, Action::Keyboard(held) if layout.qwerty_for(*held) == code)
                    .then_some(key)
            });
            let (Some(key), None) = (holders.next(), holders.next()) else {
                return false;
            };
            let Some((height, across)) = place(key) else {
                return false;
            };
            if row_height.is_some_and(|row_height| row_height != height)
                || left.is_some_and(|left| left >= across)
            {
                return false;
            }
            row_height = Some(height);
            left = Some(across);
        }
        if row_above.is_some_and(|above| row_height.is_none_or(|height| height <= above)) {
            return false;
        }
        row_above = row_height;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMNS: usize = 14;
    const QWERTY_ROWS: [&str; 4] = ["1234567890-=", "qwertyuiop[]", "asdfghjkl;'", "zxcvbnm,./"];

    fn qwerty_keymap() -> Keymap {
        let mut keymap: Keymap = (0..COLUMNS * 5).map(|_| Action::None).collect();
        for (row, keys) in QWERTY_ROWS.iter().enumerate() {
            for (column, character) in keys.chars().enumerate() {
                if let (Some(slot), Some(code)) = (
                    keymap.get_mut((row + 1) * COLUMNS + column + 1),
                    usage_of_character(character),
                ) {
                    *slot = Action::Keyboard(code);
                }
            }
        }
        if let Some(slot) = keymap.get_mut(4 * COLUMNS) {
            *slot = Action::Keyboard(0xE1);
        }
        keymap
    }

    fn place(key: usize) -> Option<(u16, u16)> {
        Some((
            u16::try_from(key / COLUMNS).ok()?,
            u16::try_from(key % COLUMNS).ok()?,
        ))
    }

    #[test]
    fn every_layout_rearranges_the_same_keys() {
        let mut qwerty: Vec<char> = QWERTY_KEYS.chars().collect();
        qwerty.sort_unstable();
        for layout in LetterLayout::ALL {
            let mut keys: Vec<char> = layout.keys().chars().collect();
            keys.sort_unstable();
            assert_eq!(keys, qwerty, "{} is not a rearrangement", layout.name());
        }
    }

    #[test]
    fn converting_there_and_back_restores_every_usage() {
        for layout in LetterLayout::ALL {
            for code in 0x04..=0xE7 {
                assert_eq!(layout.qwerty_for(layout.typed_for(code)), code);
            }
        }
    }

    #[test]
    fn dvorak_moves_the_keys_as_wootility_does() {
        assert_eq!(LetterLayout::Dvorak.typed_for(0x14), 0x34);
        assert_eq!(LetterLayout::Dvorak.typed_for(0x2D), 0x2F);
        assert_eq!(LetterLayout::Dvorak.typed_for(0x04), 0x04);
        assert_eq!(LetterLayout::Colemak.typed_for(0x16), 0x15);
        assert_eq!(LetterLayout::Workman.typed_for(0x06), 0x10);
    }

    #[test]
    fn the_layout_of_the_letter_keys_is_recognised() {
        let qwerty = qwerty_keymap();
        assert_eq!(detect(&qwerty, place), Some(LetterLayout::Qwerty));
        for layout in LetterLayout::ALL {
            let converted = convert(&qwerty, LetterLayout::Qwerty, layout);
            assert_eq!(detect(&converted, place), Some(layout));
            assert_eq!(convert(&converted, layout, LetterLayout::Qwerty), qwerty);
        }
    }

    #[test]
    fn switching_between_layouts_goes_through_qwerty() {
        let qwerty = qwerty_keymap();
        let dvorak = convert(&qwerty, LetterLayout::Qwerty, LetterLayout::Dvorak);
        assert_eq!(
            convert(&dvorak, LetterLayout::Dvorak, LetterLayout::Colemak),
            convert(&qwerty, LetterLayout::Qwerty, LetterLayout::Colemak)
        );
    }

    #[test]
    fn a_rearranged_letter_makes_the_layout_unknown() {
        let mut keymap = qwerty_keymap();
        let swapped: Vec<usize> = keymap
            .iter()
            .enumerate()
            .filter(|(_, action)| matches!(action, Action::Keyboard(0x14 | 0x1A)))
            .map(|(key, _)| key)
            .collect();
        for key in swapped {
            if let Some(slot) = keymap.get_mut(key) {
                *slot = match *slot {
                    Action::Keyboard(0x14) => Action::Keyboard(0x1A),
                    _ => Action::Keyboard(0x14),
                };
            }
        }
        assert_eq!(detect(&keymap, place), None);
    }
}
