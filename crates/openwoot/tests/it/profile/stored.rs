use embassy_futures::block_on;
use embassy_sync::mutex::Mutex;
use embassy_usb_driver::Speed as BusSpeed;
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    keys, profile,
    profile::Configurator,
    protocol::Configuration,
    storage::Scope,
};
use openwoot_protocol::{
    device,
    diagnostics::Usb,
    gamepad,
    lightbar::{self, Entry, Range, Solid, Target, Title},
    lighting::Color,
    profile::Slot,
};

use super::{FACTORY, open, storage, switch_capabilities};

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

#[test]
fn the_device_settings_tell_the_ones_in_effect_from_the_ones_of_the_next_start() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    block_on(settings.store(Scope::Global, &GAMEPAD)).unwrap();
    let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = Configurator::new(&model, switch_capabilities())
        .with_negotiated_speed(|| Some(BusSpeed::Full));
    assert_eq!(
        block_on(configurator.device_settings()),
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
    block_on(configurator.set_device_settings(xinput)).unwrap();
    block_on(configurator.save()).unwrap();
    block_on(configurator.revert()).unwrap();
    let state = block_on(configurator.device_settings()).unwrap();
    assert_eq!((state.active, state.chosen), (GAMEPAD, xinput));
    block_on(configurator.factory_reset()).unwrap();
    let state = block_on(configurator.device_settings()).unwrap();
    assert_eq!(
        (state.active, state.chosen),
        (GAMEPAD, device::Settings::from(FACTORY.face))
    );
    assert_eq!(
        block_on(Configurator::new(&model, switch_capabilities()).device_settings())
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
    assert_eq!(block_on(model.device_name()), device::Name::new());
    let desk = device::Name::try_from("Desk keyboard").unwrap();
    block_on(model.set_device_name(desk.clone())).unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(block_on(model.device_name()), desk);
    block_on(model.factory_reset()).unwrap();
    assert_eq!(block_on(model.device_name()), device::Name::new());
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
    block_on(model.switch(Slot::new(1))).unwrap();
    block_on(model.switch(Slot::new(0))).unwrap();
    block_on(model.store_active()).unwrap();
    assert_eq!(stores(), before);

    block_on(model.switch(Slot::new(1))).unwrap();
    drop(model);
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.summary().active, Slot::new(0));

    block_on(model.switch(Slot::new(1))).unwrap();
    block_on(model.store_active()).unwrap();
    block_on(model.store_active()).unwrap();
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
    block_on(model.set_gamepad_settings(Slot::new(1), kept)).unwrap();
    assert_eq!(block_on(model.gamepad_settings(Slot::new(1))), Ok(kept));
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
    block_on(model.set_lightbar(Slot::new(1), &profile_bar)).unwrap();
    let global: lightbar::Entries = [titled("Everywhere")].into_iter().collect();
    model.set_global_lightbar(&global);
    block_on(model.save()).unwrap();
    drop(model);

    let mut model = open(&keys, &shared, Some(&mut settings));
    let mut read = lightbar::Settings {
        enabled: false,
        entries: lightbar::Entries::new(),
        use_global: true,
        brightness: None,
    };
    block_on(model.lightbar(Slot::new(1), &mut read)).unwrap();
    assert_eq!(read, profile_bar);
    assert_eq!(model.global_lightbar(), &global);
}
