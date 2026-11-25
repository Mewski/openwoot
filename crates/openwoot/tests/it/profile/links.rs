use openwoot::{keys, profile};
use openwoot_protocol::{
    configuration::{Invalid, Rejection},
    keys::{Keymap, Layer},
    profile::{Namespace, Slot},
};

use super::{ProfileModel, open, storage, typed};

fn layer_of(model: &mut ProfileModel<'_>, profile: u8, layer: Layer) -> Result<Keymap, Rejection> {
    let mut keymap = Keymap::new();
    model
        .keymap(Slot::new(profile), layer, &mut keymap)
        .map(|()| keymap)
}

fn linked(model: &mut ProfileModel<'_>, profile: u8) -> Result<bool, Rejection> {
    model.keymap_link(Slot::new(profile))
}

#[test]
fn a_profile_without_its_own_keymaps_follows_the_first_profile() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(linked(&mut model, 0), Ok(false));
    assert_eq!(linked(&mut model, 1), Ok(true));

    model
        .set_keymap(Slot::new(0), Layer::One, &typed(0x05))
        .unwrap();
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));
    model.save().unwrap();
    model.activate(Slot::new(1)).unwrap();
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));

    model
        .set_keymap(Slot::new(0), Layer::Two, &typed(0x06))
        .unwrap();
    assert_eq!(layer_of(&mut model, 1, Layer::Two), Ok(typed(0x06)));
    assert_eq!(
        keys.edit(|running| running.keymaps.0[2].clone()),
        typed(0x06)
    );
    assert_eq!(linked(&mut model, 1), Ok(true));
}

#[test]
fn writing_a_layer_of_a_linked_profile_gives_it_a_copy_of_the_first_profiles_keymaps() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    model
        .set_keymap(Slot::new(0), Layer::One, &typed(0x05))
        .unwrap();
    model.save().unwrap();

    model
        .set_keymap(Slot::new(1), Layer::Two, &typed(0x07))
        .unwrap();
    assert_eq!(linked(&mut model, 1), Ok(false));
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));
    assert_eq!(layer_of(&mut model, 1, Layer::Two), Ok(typed(0x07)));

    model
        .set_keymap(Slot::new(0), Layer::One, &typed(0x08))
        .unwrap();
    model.save().unwrap();
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));

    let revision = model.revision(Slot::new(1)).unwrap();
    model.set_keymap_link(Slot::new(1), true).unwrap();
    assert_eq!(linked(&mut model, 1), Ok(true));
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x08)));
    assert_eq!(
        layer_of(&mut model, 1, Layer::Two),
        layer_of(&mut model, 0, Layer::Two)
    );
    assert_eq!(model.revision(Slot::new(1)), Ok(revision.wrapping_add(1)));
}

#[test]
fn linking_the_active_profile_is_stored_by_save_and_dropped_by_revert() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    model
        .set_keymap(Slot::new(0), Layer::One, &typed(0x05))
        .unwrap();
    model.activate(Slot::new(1)).unwrap();

    model.set_keymap_link(Slot::new(1), false).unwrap();
    assert_eq!(linked(&mut model, 1), Ok(false));
    model.revert();
    assert_eq!(linked(&mut model, 1), Ok(true));
    model.set_keymap_link(Slot::new(1), false).unwrap();
    model.save().unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.summary().active, Slot::new(1));
    assert_eq!(linked(&mut model, 1), Ok(false));
    model
        .set_keymap(Slot::new(1), Layer::One, &typed(0x09))
        .unwrap();
    model.set_keymap_link(Slot::new(1), true).unwrap();
    assert_eq!(linked(&mut model, 1), Ok(true));
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));
    model.revert();
    assert_eq!(linked(&mut model, 1), Ok(false));
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));
    model.set_keymap_link(Slot::new(1), true).unwrap();
    model.save().unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(linked(&mut model, 1), Ok(true));
}

#[test]
fn the_first_profile_is_never_linked() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(
        model.set_keymap_link(Slot::new(0), true),
        Err(Rejection::Invalid(Invalid::FirstProfileLinked))
    );
    assert_eq!(model.set_keymap_link(Slot::new(0), false), Ok(()));
    assert_eq!(
        model.set_keymap_link(Slot::new(3), true),
        Err(Rejection::ProfileMissing(3))
    );
}

#[test]
fn linked_profiles_follow_the_profile_moved_to_the_first_slot() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let created = model.create(Namespace::Onboard).unwrap();
    model
        .set_keymap(Slot::new(0), Layer::One, &typed(0x05))
        .unwrap();
    model.save().unwrap();
    model
        .set_keymap(Slot::new(1), Layer::One, &typed(0x06))
        .unwrap();
    assert_eq!(
        layer_of(&mut model, created.get(), Layer::One),
        Ok(typed(0x05))
    );

    model.shift(Slot::new(1), Slot::new(0)).unwrap();
    assert_eq!(
        layer_of(&mut model, created.get(), Layer::One),
        Ok(typed(0x06))
    );
    assert_eq!(linked(&mut model, 1), Ok(false));
    assert_eq!(layer_of(&mut model, 1, Layer::One), Ok(typed(0x05)));
    assert_eq!(
        keys.edit(|running| running.keymaps.0[1].clone()),
        typed(0x05)
    );
}
