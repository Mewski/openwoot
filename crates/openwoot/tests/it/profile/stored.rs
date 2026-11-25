use core::cell::RefCell;

use embassy_usb_driver::Speed as BusSpeed;
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    keys::{self, ProfileSource},
    profile,
    profile::{Configurator, Model},
    protocol::Configuration,
    storage::{LayoutOverride, Record, RecordKey, Scope, Settings},
};
use openwoot_protocol::{
    advanced,
    apps::{self, App, Link},
    configuration::Rejection,
    device,
    diagnostics::Usb,
    gamepad, global,
    heapless::Vec,
    keys::{Keymap, Layer, Overrides},
    lightbar::{self, Entry, Range, Solid, Target, Title},
    lighting::{self, Color},
    profile::{Info, Namespace, Slot},
    switches,
};

use super::{
    FACTORY, Mutual, ProbedModel, ProfileModel, open, open_probed, probed_storage, storage,
    switch_capabilities,
};
use crate::storage::Nor;

const GAMEPAD: device::Settings = device::Settings {
    mode: device::Mode::Gamepad,
    keyboard: device::KeyboardFormat::NKeyRollover,
    speed: device::Speed::High,
    gamepad_polling: device::PollingRate::Every1Millisecond,
};

fn titled(title: &str) -> Entry {
    Entry {
        title: Title::try_from(title).unwrap_or_default(),
        condition: None,
        effect: lightbar::Effect::Solid(Solid {
            color: Color {
                red: 1,
                green: 2,
                blue: 3,
                alpha: 255,
            },
            target: Target::Lightbar(Range { first: 0, last: 3 }),
        }),
    }
}

fn undecodable<R: Record>(settings: &mut Settings<Nor>, scope: Scope) {
    let stored = settings.store_with(RecordKey::of::<R>(scope), R::VERSION, &|_| Some(0));
    assert_eq!(stored, Ok(()));
}

fn empty_lightbar() -> lightbar::Settings {
    lightbar::Settings {
        enabled: false,
        entries: lightbar::Entries::new(),
        use_global: true,
        brightness: None,
    }
}

fn empty_mapping() -> gamepad::Mapping {
    gamepad::Mapping {
        bindings: Vec::new(),
        exclusive: true,
    }
}

fn assert_same_parts(model: &mut ProfileModel<'_>, factory: &mut ProfileModel<'_>, profile: Slot) {
    assert_eq!(model.settings(profile), factory.settings(profile));
    assert_eq!(
        model.gamepad_settings(profile),
        factory.gamepad_settings(profile)
    );
    assert_eq!(model.lighting(profile), factory.lighting(profile));
    assert_eq!(model.info(profile), factory.info(profile));
    assert_eq!(model.revision(profile), factory.revision(profile));
    let (mut stored, mut expected) = (Overrides::new(), Overrides::new());
    assert_eq!(model.overrides(profile, &mut stored), Ok(()));
    assert_eq!(factory.overrides(profile, &mut expected), Ok(()));
    assert_eq!(stored, expected);
    let (mut stored, mut expected) = (advanced::Keys::new(), advanced::Keys::new());
    assert_eq!(model.advanced(profile, &mut stored), Ok(()));
    assert_eq!(factory.advanced(profile, &mut expected), Ok(()));
    assert_eq!(stored, expected);
    let (mut stored, mut expected) = (empty_mapping(), empty_mapping());
    assert_eq!(model.gamepad_mapping(profile, &mut stored), Ok(()));
    assert_eq!(factory.gamepad_mapping(profile, &mut expected), Ok(()));
    assert_eq!(stored, expected);
    let (mut stored, mut expected) = (empty_lightbar(), empty_lightbar());
    assert_eq!(model.lightbar(profile, &mut stored), Ok(()));
    assert_eq!(factory.lightbar(profile, &mut expected), Ok(()));
    assert_eq!(stored, expected);
    for layer in Layer::ALL {
        let (mut stored, mut expected) = (Keymap::new(), Keymap::new());
        assert_eq!(model.keymap(profile, layer, &mut stored), Ok(()));
        assert_eq!(factory.keymap(profile, layer, &mut expected), Ok(()));
        assert_eq!(stored, expected, "{layer:?}");
        let (mut stored, mut expected) = (None, None);
        assert_eq!(model.key_colors(profile, layer, &mut stored), Ok(()));
        assert_eq!(factory.key_colors(profile, layer, &mut expected), Ok(()));
        assert_eq!(stored, expected, "{layer:?}");
    }
}

#[test]
fn stored_records_that_do_not_decode_give_way_to_the_factory_ones() {
    let mut settings = storage();
    for slot in [0, 1] {
        let scope = Scope::Profile(slot);
        undecodable::<openwoot_protocol::keys::Settings>(&mut settings, scope);
        undecodable::<keys::Keymaps>(&mut settings, scope);
        undecodable::<Overrides>(&mut settings, scope);
        undecodable::<advanced::Keys>(&mut settings, scope);
        undecodable::<gamepad::Mapping>(&mut settings, scope);
        undecodable::<gamepad::Settings>(&mut settings, scope);
        undecodable::<lighting::Settings>(&mut settings, scope);
        undecodable::<profile::Colors>(&mut settings, scope);
        undecodable::<Info>(&mut settings, scope);
        undecodable::<profile::Revision>(&mut settings, scope);
        undecodable::<lightbar::Settings>(&mut settings, scope);
    }
    undecodable::<keys::ActiveProfile>(&mut settings, Scope::Global);
    undecodable::<profile::Count>(&mut settings, Scope::Global);
    undecodable::<profile::LinkedCount>(&mut settings, Scope::Global);
    undecodable::<profile::Order>(&mut settings, Scope::Global);
    undecodable::<global::Settings>(&mut settings, Scope::Global);
    undecodable::<device::Settings>(&mut settings, Scope::Global);
    undecodable::<lightbar::Entries>(&mut settings, Scope::Global);
    undecodable::<switches::Settings>(&mut settings, Scope::Global);
    undecodable::<profile::DeviceName>(&mut settings, Scope::Global);
    undecodable::<LayoutOverride>(&mut settings, Scope::Global);
    let (keys, shared) = (keys::Shared::new(), profile::Shared::new());
    let mut model = open(&keys, &shared, Some(&mut settings));
    let (factory_keys, factory_shared) = (keys::Shared::new(), profile::Shared::new());
    let mut factory = open(&factory_keys, &factory_shared, None);
    assert_eq!(model.summary(), factory.summary());
    assert_eq!(keys.read(Clone::clone), factory_keys.read(Clone::clone));
    assert_eq!(shared.read(Clone::clone), factory_shared.read(Clone::clone));
    assert_eq!(model.global_settings(), factory.global_settings());
    assert_eq!(model.device_settings(), factory.device_settings());
    assert_eq!(model.global_lightbar(), factory.global_lightbar());
    assert_eq!(model.switches(), factory.switches());
    assert_eq!(model.chosen_variant(), factory.chosen_variant());
    assert_eq!(model.device_name(), factory.device_name());
    assert_same_parts(&mut model, &mut factory, Slot::new(1));
}

fn storage_counts(diagnostics: &Diagnostics) -> [u32; 3] {
    let storage = diagnostics
        .report(Usb::default(), Hardware::default())
        .storage;
    [storage.loads, storage.stores, storage.failures]
}

fn read_and_write_every_part(
    model: &mut ProbedModel<'_>,
    profile: Slot,
    note: &mut impl FnMut(&'static str),
) {
    let settings = model.settings(profile);
    let written = settings.map(|settings| model.set_settings(profile, settings));
    assert_eq!(written, Ok(Ok(())));
    note("settings");
    let mut overrides = Overrides::new();
    assert_eq!(model.overrides(profile, &mut overrides), Ok(()));
    assert_eq!(model.set_overrides(profile, &overrides), Ok(()));
    note("overrides");
    let mut advanced_keys = advanced::Keys::new();
    assert_eq!(model.advanced(profile, &mut advanced_keys), Ok(()));
    assert_eq!(model.set_advanced(profile, &advanced_keys), Ok(()));
    note("advanced keys");
    let mut mapping = empty_mapping();
    assert_eq!(model.gamepad_mapping(profile, &mut mapping), Ok(()));
    assert_eq!(model.set_gamepad_mapping(profile, &mapping), Ok(()));
    note("gamepad mapping");
    let gamepad = model.gamepad_settings(profile);
    let written = gamepad.map(|gamepad| model.set_gamepad_settings(profile, gamepad));
    assert_eq!(written, Ok(Ok(())));
    note("gamepad settings");
    let lighting = model.lighting(profile);
    let written = lighting.map(|lighting| model.set_lighting(profile, lighting));
    assert_eq!(written, Ok(Ok(())));
    note("lighting");
    let mut lightbar = empty_lightbar();
    assert_eq!(model.lightbar(profile, &mut lightbar), Ok(()));
    assert_eq!(model.set_lightbar(profile, &lightbar), Ok(()));
    note("light bar");
    let mut colors = None;
    assert_eq!(model.key_colors(profile, Layer::One, &mut colors), Ok(()));
    let written = model.set_key_colors(profile, Layer::One, colors.as_ref());
    assert_eq!(written, Ok(()));
    note("key colours");
    let mut keymap = Keymap::new();
    assert_eq!(model.keymap(profile, Layer::One, &mut keymap), Ok(()));
    assert_eq!(model.set_keymap(profile, Layer::One, &keymap), Ok(()));
    note("keymap");
    let info = model.info(profile);
    let written = info.map(|info| model.set_info(profile, info));
    assert_eq!(written, Ok(Ok(())));
    assert!(model.revision(profile).is_ok());
    note("info");
}

#[test]
fn reading_and_writing_profile_parts_counts_each_record_once() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let mut seen = std::vec::Vec::new();
    let mut note = |step: &'static str| seen.push((step, storage_counts(&DIAGNOSTICS)));
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let (mut settings, _) = probed_storage();
    settings.with_diagnostics(&DIAGNOSTICS);
    let mut model = open_probed(&keys, &shared, &mut settings);
    note("load");
    for profile in [Slot::new(1), Slot::new(0)] {
        read_and_write_every_part(&mut model, profile, &mut note);
    }
    model.set_keymap_link(Slot::new(1), true).unwrap();
    note("keymap link");
    model
        .set_device_name(device::Name::try_from("Desk keyboard").unwrap())
        .unwrap();
    note("device name");
    model.set_global_settings(model.global_settings());
    model.set_device_settings(model.device_settings());
    let entries = model.global_lightbar().clone();
    model.set_global_lightbar(&entries);
    let switches = model.switches();
    model.set_switches(&switches).unwrap();
    model.set_layout_variant(0);
    model.save().unwrap();
    note("save");
    shared.edit(|appearance| appearance.settings.brightness = 7);
    model.save_brightness().unwrap();
    note("brightness");
    assert_eq!(
        seen,
        [
            ("load", [19, 0, 0]),
            ("settings", [21, 2, 0]),
            ("overrides", [23, 4, 0]),
            ("advanced keys", [25, 6, 0]),
            ("gamepad mapping", [27, 8, 0]),
            ("gamepad settings", [29, 10, 0]),
            ("lighting", [31, 12, 0]),
            ("light bar", [33, 14, 0]),
            ("key colours", [36, 16, 0]),
            ("keymap", [39, 18, 0]),
            ("info", [42, 20, 0]),
            ("settings", [42, 20, 0]),
            ("overrides", [42, 20, 0]),
            ("advanced keys", [42, 20, 0]),
            ("gamepad mapping", [42, 20, 0]),
            ("gamepad settings", [42, 20, 0]),
            ("lighting", [42, 20, 0]),
            ("light bar", [42, 20, 0]),
            ("key colours", [42, 20, 0]),
            ("keymap", [42, 20, 0]),
            ("info", [43, 20, 0]),
            ("keymap link", [45, 22, 0]),
            ("device name", [46, 23, 0]),
            ("save", [47, 39, 0]),
            ("brightness", [49, 41, 0]),
        ]
    );
}

#[test]
fn managing_profiles_counts_each_record_once() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let mut seen = std::vec::Vec::new();
    let mut note = |step: &'static str| seen.push((step, storage_counts(&DIAGNOSTICS)));
    let keys = keys::Shared::<Mutual>::new();
    let shared = profile::Shared::new();
    let (mut settings, watch) = probed_storage();
    settings.with_diagnostics(&DIAGNOSTICS);
    let mut model: ProbedModel<'_> =
        Model::new(&keys, &shared, Some(&mut settings), &FACTORY, 0, 4).with_linked(2);
    model.load();
    note("load");
    model.activate(Slot::new(1)).unwrap();
    note("activate");
    model.swap(Slot::new(0), Slot::new(1)).unwrap();
    note("swap");
    let created = model.create(Namespace::Onboard).unwrap();
    model.delete(created).unwrap();
    note("create and delete");
    let linked = model.create(Namespace::Linked).unwrap();
    let link = Link {
        name: apps::Text::try_from("game").unwrap(),
        app: App::Steam(7),
    };
    model.set_app_link(linked, 0, &link).unwrap();
    model.set_app_link(linked, 1, &link).unwrap();
    let mut read = link.clone();
    model.app_link(linked, 1, &mut read).unwrap();
    model.delete_app_link(linked, 0).unwrap();
    note("app links");
    model.storage_usage().unwrap();
    note("usage");
    let inactive = if model.summary().active == Slot::new(0) {
        Slot::new(1)
    } else {
        Slot::new(0)
    };
    watch.fail_writes(true);
    let info = model.info(inactive).unwrap();
    assert_eq!(
        model.set_info(inactive, info),
        Err(Rejection::StorageInaccessible)
    );
    note("refused write");
    watch.fail_writes(false);
    model.factory_reset().unwrap();
    note("factory reset");
    assert_eq!(
        seen,
        [
            ("load", [19, 0, 0]),
            ("activate", [30, 1, 0]),
            ("swap", [31, 2, 0]),
            ("create and delete", [31, 26, 0]),
            ("app links", [42, 61, 0]),
            ("usage", [43, 61, 0]),
            ("refused write", [44, 61, 1]),
            ("factory reset", [62, 63, 1]),
        ]
    );
}

#[test]
fn the_device_settings_tell_the_ones_in_effect_from_the_ones_of_the_next_start() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    settings.store(Scope::Global, &GAMEPAD).unwrap();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = Configurator::new(&model, switch_capabilities())
        .with_negotiated_speed(|| Some(BusSpeed::Full));
    assert_eq!(
        configurator.device_settings(),
        Ok(device::SettingsState {
            active: GAMEPAD,
            chosen: GAMEPAD,
            negotiated: Some(device::Speed::Full),
        })
    );

    let xinput = device::Settings {
        mode: device::Mode::Xinput,
        ..GAMEPAD
    };
    configurator.set_device_settings(xinput).unwrap();
    configurator.save().unwrap();
    configurator.revert().unwrap();
    let state = configurator.device_settings().unwrap();
    assert_eq!((state.active, state.chosen), (GAMEPAD, xinput));
    configurator.factory_reset().unwrap();
    let state = configurator.device_settings().unwrap();
    assert_eq!(
        (state.active, state.chosen),
        (GAMEPAD, device::Settings::from(FACTORY.face))
    );
    assert_eq!(
        Configurator::new(&model, switch_capabilities())
            .device_settings()
            .map(|state| state.negotiated),
        Ok(None)
    );
}

#[test]
fn the_device_name_is_stored_at_once_and_cleared_by_a_factory_reset() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.device_name(), device::Name::new());
    let desk = device::Name::try_from("Desk keyboard").unwrap();
    model.set_device_name(desk.clone()).unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.device_name(), desk);
    model.factory_reset().unwrap();
    assert_eq!(model.device_name(), device::Name::new());
}

#[test]
fn a_switch_by_key_stores_the_active_profile_once_and_only_when_it_changed() {
    static DIAGNOSTICS: Diagnostics = Diagnostics::new();
    let stores = || {
        DIAGNOSTICS
            .report(Usb::default(), Hardware::default())
            .storage
            .stores
    };
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    settings.with_diagnostics(&DIAGNOSTICS);
    let mut model = open(&keys, &shared, Some(&mut settings));
    let before = stores();
    model.switch(Slot::new(1)).unwrap();
    model.switch(Slot::new(0)).unwrap();
    model.store_active().unwrap();
    assert_eq!(stores(), before);

    model.switch(Slot::new(1)).unwrap();
    drop(model);
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.summary().active, Slot::new(0));

    model.switch(Slot::new(1)).unwrap();
    model.store_active().unwrap();
    model.store_active().unwrap();
    assert_eq!(stores(), before + 1);
    drop(model);
    let model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.summary().active, Slot::new(1));
}

#[test]
fn a_disabled_stick_bias_keeps_its_percentage() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let kept = gamepad::Settings {
        left_stick_up_bias: gamepad::StickBias {
            enabled: false,
            percent: 30,
        },
        ..keys::default_gamepad_settings()
    };
    model.set_gamepad_settings(Slot::new(1), kept).unwrap();
    assert_eq!(model.gamepad_settings(Slot::new(1)), Ok(kept));
}

#[test]
fn light_bar_titles_stay_with_their_entries() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let profile_bar = lightbar::Settings {
        enabled: true,
        entries: [titled("Caps"), titled("Quiet hours")]
            .into_iter()
            .collect(),
        use_global: false,
        brightness: None,
    };
    model.set_lightbar(Slot::new(1), &profile_bar).unwrap();
    let global: lightbar::Entries = [titled("Everywhere")].into_iter().collect();
    model.set_global_lightbar(&global);
    model.save().unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    let mut read = lightbar::Settings {
        enabled: false,
        entries: lightbar::Entries::new(),
        use_global: true,
        brightness: None,
    };
    model.lightbar(Slot::new(1), &mut read).unwrap();
    assert_eq!(read, profile_bar);
    assert_eq!(model.global_lightbar(), &global);
}
