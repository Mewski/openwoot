use heapless::Vec;
use openwoot_protocol::{
    advanced, gamepad, keys,
    wire::{Encode, Writer},
};

use crate::{
    keys::{Keymaps, Profile, ProfileSource},
    layout::LAYERS,
    storage::Record,
};

pub(super) trait KeyPart: Record {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()>;

    fn encode_part(source: &impl ProfileSource, buffer: &mut [u8]) -> Option<usize> {
        let mut writer = Writer::new(buffer);
        Self::write(source, &mut writer)?;
        writer.finish()
    }
}

impl KeyPart for keys::Settings {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        source.read(|profile| profile.settings)?.encode(writer);
        Some(())
    }
}

impl KeyPart for Keymaps {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        (0..LAYERS).try_for_each(|layer| {
            write_sequence(source, writer, |profile| profile.keymaps.0.get(layer))
        })
    }
}

impl KeyPart for keys::Overrides {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        write_sequence(source, writer, |profile| Some(&profile.overrides))
    }
}

impl KeyPart for advanced::Keys {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        write_sequence(source, writer, |profile| Some(&profile.advanced))
    }
}

impl KeyPart for gamepad::Mapping {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        write_sequence(source, writer, |profile| {
            Some(&profile.gamepad_mapping.bindings)
        })?;
        source
            .read(|profile| profile.gamepad_mapping.exclusive)?
            .encode(writer);
        Some(())
    }
}

impl KeyPart for gamepad::Settings {
    fn write(source: &impl ProfileSource, writer: &mut Writer<'_>) -> Option<()> {
        source
            .read(|profile| profile.gamepad_settings)?
            .encode(writer);
        Some(())
    }
}

fn write_sequence<T: Copy + Encode, const N: usize>(
    source: &impl ProfileSource,
    writer: &mut Writer<'_>,
    sequence: impl Fn(&Profile) -> Option<&Vec<T, N>>,
) -> Option<()> {
    let length = source.read(|profile| sequence(profile).map(|entries| entries.len()))??;
    length.encode(writer);
    (0..length).try_for_each(|index| {
        source
            .read(|profile| sequence(profile)?.get(index).copied())??
            .encode(writer);
        Some(())
    })
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use openwoot_protocol::{
        advanced::Behaviour,
        gamepad::Control,
        keys::{Action, Distance, Index, Layer, Override},
        wire,
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
        let length = T::encode_part(source, &mut pieces).unwrap();
        let expected = wire::encode(part, &mut whole).unwrap();
        assert_eq!(pieces.get(..length), whole.get(..expected));
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
        assert_eq!(Keymaps::encode_part(&source, &mut body), None);
    }
}
