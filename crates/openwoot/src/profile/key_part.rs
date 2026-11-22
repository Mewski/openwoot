use heapless::Vec;
use openwoot_protocol::{advanced, gamepad, keys};
use serde::Serialize;

use crate::{
    keys::{Keymaps, Profile, ProfileSource},
    layout::LAYERS,
    storage::Record,
};

pub(super) trait KeyPart: Record {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()>;

    fn encode(source: &impl ProfileSource, buffer: &mut [u8]) -> Option<usize> {
        let mut body = Body { buffer, length: 0 };
        Self::write(source, &mut body)?;
        Some(body.length)
    }
}

pub(super) struct Body<'a> {
    buffer: &'a mut [u8],
    length: usize,
}

impl KeyPart for keys::Settings {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        body.push(&source.read(|profile| profile.settings)?)
    }
}

impl KeyPart for Keymaps {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        (0..LAYERS).try_for_each(|layer| {
            body.push_sequence(source, |profile| profile.keymaps.0.get(layer))
        })
    }
}

impl KeyPart for keys::Overrides {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        body.push_sequence(source, |profile| Some(&profile.overrides))
    }
}

impl KeyPart for advanced::Keys {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        body.push_sequence(source, |profile| Some(&profile.advanced))
    }
}

impl KeyPart for gamepad::Mapping {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        body.push_sequence(source, |profile| Some(&profile.gamepad_mapping.bindings))?;
        body.push(&source.read(|profile| profile.gamepad_mapping.exclusive)?)
    }
}

impl KeyPart for gamepad::Settings {
    fn write(source: &impl ProfileSource, body: &mut Body<'_>) -> Option<()> {
        body.push(&source.read(|profile| profile.gamepad_settings)?)
    }
}

impl Body<'_> {
    fn push(&mut self, value: &impl Serialize) -> Option<()> {
        let rest = self.buffer.get_mut(self.length..)?;
        let written = postcard::to_slice(value, rest).ok()?.len();
        self.length = self.length.checked_add(written)?;
        Some(())
    }

    fn push_sequence<T: Copy + Serialize, const N: usize>(
        &mut self,
        source: &impl ProfileSource,
        sequence: impl Fn(&Profile) -> Option<&Vec<T, N>>,
    ) -> Option<()> {
        let length = source.read(|profile| sequence(profile).map(|entries| entries.len()))??;
        self.push(&length)?;
        (0..length).try_for_each(|index| {
            let entry = source.read(|profile| sequence(profile)?.get(index).copied())??;
            self.push(&entry)
        })
    }
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use openwoot_protocol::{
        advanced::Behaviour,
        gamepad::Control,
        keys::{Action, Distance, Index, Layer, Override},
    };

    use super::*;

    const KEYS: usize = 6;

    struct Counted {
        profile: Profile,
        reads: Cell<usize>,
        largest: Cell<usize>,
    }

    impl ProfileSource for Counted {
        fn read<R>(&self, read: impl FnOnce(&Profile) -> R) -> Option<R> {
            self.reads.set(self.reads.get() + 1);
            self.largest.set(self.largest.get().max(size_of::<R>()));
            Some(read(&self.profile))
        }
    }

    fn full_profile() -> Profile {
        let mut profile = Profile::empty();
        for (layer, keymap) in (0u8..).zip(profile.keymaps.0.iter_mut()) {
            keymap.extend(
                (0u8..)
                    .take(KEYS)
                    .map(|key| Action::Keyboard(layer * 16 + key)),
            );
        }
        profile.overrides.extend(core::iter::repeat_n(
            Override {
                actuation: Some(Distance::from_fraction(0x8000)),
                ..Override::default()
            },
            KEYS,
        ));
        profile.advanced.extend((0u8..2).map(|key| advanced::Key {
            key: Index::new(key),
            layer: Layer::Base,
            behaviour: Behaviour::Toggle(Action::Keyboard(0x04)),
        }));
        profile
            .gamepad_mapping
            .bindings
            .extend(core::iter::repeat_n(Some(Control::ButtonA), KEYS));
        profile.gamepad_mapping.exclusive = true;
        profile
    }

    fn assert_encodes_like_the_whole_part<T: KeyPart>(source: &Counted, part: &T) {
        let mut pieces = [0; 2048];
        let mut whole = [0; 2048];
        let length = T::encode(source, &mut pieces).unwrap();
        let expected = postcard::to_slice(part, &mut whole).unwrap();
        assert_eq!(pieces.get(..length), Some(&*expected));
    }

    #[test]
    fn saving_key_parts_copies_at_most_one_entry_per_critical_section() {
        let source = Counted {
            profile: full_profile(),
            reads: Cell::new(0),
            largest: Cell::new(0),
        };
        let profile = &source.profile;
        assert_encodes_like_the_whole_part(&source, &profile.settings);
        assert_encodes_like_the_whole_part(&source, &profile.keymaps);
        assert_encodes_like_the_whole_part(&source, &profile.overrides);
        assert_encodes_like_the_whole_part(&source, &profile.advanced);
        assert_encodes_like_the_whole_part(&source, &profile.gamepad_mapping);
        assert_encodes_like_the_whole_part(&source, &profile.gamepad_settings);

        let entries = LAYERS * KEYS + KEYS + profile.advanced.len() + KEYS;
        assert!(source.reads.get() >= entries);
        assert!(source.largest.get() <= size_of::<Option<advanced::Key>>());
    }

    #[test]
    fn a_part_too_large_for_the_body_encodes_nothing() {
        let source = Counted {
            profile: full_profile(),
            reads: Cell::new(0),
            largest: Cell::new(0),
        };
        let mut body = [0; 8];
        assert_eq!(Keymaps::encode(&source, &mut body), None);
    }
}
