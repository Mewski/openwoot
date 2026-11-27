use openwoot_protocol::device::Layout;

const CHARACTERS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    pub usage: u8,
    pub shift: bool,
}

struct Table([Option<Stroke>; CHARACTERS]);

static ANSI: Table = Table::us();
static ISO: Table = Table::us()
    .with(b'"', 0x1F, true)
    .with(b'@', 0x34, true)
    .with(b'#', 0x31, false)
    .with(b'~', 0x31, true)
    .with(b'\\', 0x64, false)
    .with(b'|', 0x64, true);
static JIS: Table = Table::us()
    .with(b'"', 0x1F, true)
    .with(b'&', 0x23, true)
    .with(b'\'', 0x24, true)
    .with(b'(', 0x25, true)
    .with(b')', 0x26, true)
    .with(b'=', 0x2D, true)
    .with(b'^', 0x2E, false)
    .with(b'~', 0x2E, true)
    .with(b'@', 0x2F, false)
    .with(b'`', 0x2F, true)
    .with(b'[', 0x30, false)
    .with(b'{', 0x30, true)
    .with(b']', 0x31, false)
    .with(b'}', 0x31, true)
    .with(b'+', 0x33, true)
    .with(b':', 0x34, false)
    .with(b'*', 0x34, true)
    .with(b'\\', 0x87, false)
    .with(b'_', 0x87, true)
    .with(b'|', 0x89, true);

impl Stroke {
    pub const SHIFT: u8 = 0xE1;

    #[must_use]
    pub fn of(layout: Layout, character: u8) -> Option<Self> {
        let table = match layout {
            Layout::Ansi => &ANSI,
            Layout::Iso => &ISO,
            Layout::Jis => &JIS,
        };
        table.0.get(usize::from(character)).copied().flatten()
    }
}

impl Table {
    const fn us() -> Self {
        let mut table = Self([None; CHARACTERS]);
        let mut letter = 0;
        while letter < 26 {
            table = table.with(b'a' + letter, 0x04 + letter, false).with(
                b'A' + letter,
                0x04 + letter,
                true,
            );
            letter += 1;
        }
        let mut digit = 0;
        while digit < 9 {
            table = table.with(b'1' + digit, 0x1E + digit, false);
            digit += 1;
        }
        table
            .with(b'0', 0x27, false)
            .with(b'!', 0x1E, true)
            .with(b'@', 0x1F, true)
            .with(b'#', 0x20, true)
            .with(b'$', 0x21, true)
            .with(b'%', 0x22, true)
            .with(b'^', 0x23, true)
            .with(b'&', 0x24, true)
            .with(b'*', 0x25, true)
            .with(b'(', 0x26, true)
            .with(b')', 0x27, true)
            .with(b'\n', 0x28, false)
            .with(b'\t', 0x2B, false)
            .with(b' ', 0x2C, false)
            .with(b'-', 0x2D, false)
            .with(b'_', 0x2D, true)
            .with(b'=', 0x2E, false)
            .with(b'+', 0x2E, true)
            .with(b'[', 0x2F, false)
            .with(b'{', 0x2F, true)
            .with(b']', 0x30, false)
            .with(b'}', 0x30, true)
            .with(b'\\', 0x31, false)
            .with(b'|', 0x31, true)
            .with(b';', 0x33, false)
            .with(b':', 0x33, true)
            .with(b'\'', 0x34, false)
            .with(b'"', 0x34, true)
            .with(b'`', 0x35, false)
            .with(b'~', 0x35, true)
            .with(b',', 0x36, false)
            .with(b'<', 0x36, true)
            .with(b'.', 0x37, false)
            .with(b'>', 0x37, true)
            .with(b'/', 0x38, false)
            .with(b'?', 0x38, true)
    }

    #[expect(
        clippy::indexing_slicing,
        reason = "evaluated at compile time, where an index outside the table fails the build"
    )]
    const fn with(mut self, character: u8, usage: u8, shift: bool) -> Self {
        self.0[character as usize] = Some(Stroke { usage, shift });
        self
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{keys::Action, macros::is_typable};

    use super::*;

    const LAYOUTS: [Layout; 3] = [Layout::Ansi, Layout::Iso, Layout::Jis];

    fn typed(layout: Layout, characters: &[u8]) -> [Option<(u8, bool)>; 4] {
        let mut strokes = [None; 4];
        for (stroke, &character) in strokes.iter_mut().zip(characters) {
            *stroke = Stroke::of(layout, character).map(|stroke| (stroke.usage, stroke.shift));
        }
        strokes
    }

    #[test]
    fn every_typable_character_has_a_stroke_the_keyboard_report_carries_in_every_layout() {
        for layout in LAYOUTS {
            for character in 0..=0x7F_u8 {
                let stroke = Stroke::of(layout, character);
                assert_eq!(
                    stroke.is_some(),
                    is_typable(char::from(character)),
                    "{character:#04x} on {layout:?}"
                );
                if let Some(stroke) = stroke {
                    assert!(
                        Action::KEYBOARD_USAGES
                            .iter()
                            .any(|usages| usages.contains(&stroke.usage)),
                        "{character:#04x} on {layout:?}"
                    );
                }
            }
            assert_eq!(Stroke::of(layout, 0xE9), None);
        }
    }

    #[test]
    fn letters_digits_and_whitespace_sit_on_the_same_keys_in_every_layout() {
        for layout in LAYOUTS {
            assert_eq!(
                typed(layout, b"aZ0\n"),
                [
                    Some((0x04, false)),
                    Some((0x1D, true)),
                    Some((0x27, false)),
                    Some((0x28, false))
                ]
            );
            assert_eq!(
                typed(layout, b"\t ?."),
                [
                    Some((0x2B, false)),
                    Some((0x2C, false)),
                    Some((0x38, true)),
                    Some((0x37, false))
                ]
            );
        }
    }

    #[test]
    fn the_uk_layout_moves_the_quote_at_hash_and_backslash_keys() {
        assert_eq!(
            typed(Layout::Ansi, b"\"@#\\"),
            [
                Some((0x34, true)),
                Some((0x1F, true)),
                Some((0x20, true)),
                Some((0x31, false))
            ]
        );
        assert_eq!(
            typed(Layout::Iso, b"\"@#\\"),
            [
                Some((0x1F, true)),
                Some((0x34, true)),
                Some((0x31, false)),
                Some((0x64, false))
            ]
        );
        assert_eq!(
            typed(Layout::Iso, b"~|`'"),
            [
                Some((0x31, true)),
                Some((0x64, true)),
                Some((0x35, false)),
                Some((0x34, false))
            ]
        );
    }

    #[test]
    fn the_japanese_layout_shifts_the_symbols_of_its_own_keys() {
        assert_eq!(
            typed(Layout::Jis, b"&'()"),
            [
                Some((0x23, true)),
                Some((0x24, true)),
                Some((0x25, true)),
                Some((0x26, true))
            ]
        );
        assert_eq!(
            typed(Layout::Jis, b"=^~@"),
            [
                Some((0x2D, true)),
                Some((0x2E, false)),
                Some((0x2E, true)),
                Some((0x2F, false))
            ]
        );
        assert_eq!(
            typed(Layout::Jis, b":*+_"),
            [
                Some((0x34, false)),
                Some((0x34, true)),
                Some((0x33, true)),
                Some((0x87, true))
            ]
        );
        assert_eq!(
            typed(Layout::Jis, b"\\|[}"),
            [
                Some((0x87, false)),
                Some((0x89, true)),
                Some((0x30, false)),
                Some((0x31, true))
            ]
        );
    }
}
