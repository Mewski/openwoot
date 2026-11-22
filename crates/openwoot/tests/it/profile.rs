use std::pin::pin;

use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_usb_driver::Speed as BusSpeed;
use openwoot::{
    keys::{self, ProfileSource},
    layout::DefaultKeymap,
    lighting::{LedMatrix, Lighting, from_rgb565},
    profile::{self, Changes, Factory, Model, Preset},
    runtime::{Firmware, State},
    storage::{NorLayout, Partition, Settings},
    usb::{Face, KeyboardFormat, Mode, PollingRate, Speed},
};
use openwoot_image::Region;
use openwoot_protocol::{
    configuration::Rejection,
    device, global,
    heapless::Vec,
    keys::{Action, Distance, Keymap, Layer, RapidTrigger, ScanMode, TapDuration},
    lighting::{
        Color, Effect, HueRange, Idle, Indicators, KeyColors, RainbowWave, ReactiveColoring,
        ReactiveFade, Rgb, Settings as LightingSettings,
    },
    profile::{Info, Slot},
};

use crate::{
    lighting::{COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT},
    storage::{Nor, SECTOR},
    support::{Host, IDENTITY, Outcome, poll, setup},
};

type Mutual = CriticalSectionRawMutex;
type ProfileModel<'a> = Model<'a, Mutual, Nor, ROWS, COLUMNS>;

const SET_CONFIGURATION: u8 = 0x09;
const WHITE: Color = Color {
    red: 255,
    green: 255,
    blue: 255,
    alpha: 255,
};
const INDICATORS: Indicators = Indicators {
    caps_lock: WHITE,
    scroll_lock: WHITE,
    num_lock: WHITE,
    layer_lock: WHITE,
};
const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};
const BLUE: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 255,
};
const TYPING: openwoot_protocol::keys::Settings = openwoot_protocol::keys::Settings {
    actuation: Distance::from_fraction(63 * 257),
    rapid_trigger_enabled: false,
    rapid_trigger: RapidTrigger {
        press: Distance::from_fraction(65 * 257),
        release: None,
        continuous: false,
    },
    tap_duration: TapDuration::FortyMilliseconds,
    scan_mode: ScanMode::Single,
    keyboard_in_gamepad_mode: true,
};
const LAYERS: [[[Action; COLUMNS]; ROWS]; 1] = [[[Action::Keyboard(0x04); COLUMNS]; ROWS]];
static BLUE_KEYS: KeyColors = Vec::from_array([BLUE; ROWS * COLUMNS]);
static PRESETS: [Preset; 2] = [
    Preset {
        name: "Typing Profile",
        color: BLUE,
        keys: TYPING,
        actuation_points: &[],
        advanced: &[],
        lighting: LightingSettings {
            brightness: 255,
            indicators: INDICATORS,
            effects: Vec::from_array([
                Effect::RainbowWave(RainbowWave {
                    hues: HueRange::From113To204,
                    period: 34,
                    forward: false,
                }),
                Effect::ReactiveFade(ReactiveFade {
                    base_level: 255,
                    step_ms: 8,
                    coloring: ReactiveColoring::Fixed(WHITE),
                    inverted: false,
                }),
            ]),
        },
        colors: [Some(&BLUE_KEYS), None, None, None],
        lightbar_brightness: None,
    },
    Preset {
        name: "Rapid Profile",
        color: BLACK,
        keys: openwoot_protocol::keys::Settings {
            rapid_trigger_enabled: true,
            ..TYPING
        },
        actuation_points: &[],
        advanced: &[],
        lighting: LightingSettings {
            brightness: 128,
            indicators: INDICATORS,
            effects: Vec::new(),
        },
        colors: [None, None, None, None],
        lightbar_brightness: Some(90),
    },
];
static FACTORY: Factory<ROWS, COLUMNS> = Factory {
    keymap: DefaultKeymap {
        layers: &LAYERS,
        variant_patches: &[],
    },
    presets: &PRESETS,
    fallback: Preset {
        name: "",
        color: BLACK,
        keys: TYPING,
        actuation_points: &[],
        advanced: &[],
        lighting: LightingSettings {
            brightness: 255,
            indicators: INDICATORS,
            effects: Vec::new(),
        },
        colors: [None, None, None, None],
        lightbar_brightness: None,
    },
    created: 2,
    global: global::Settings {
        idle: Some(Idle {
            dim_after_minutes: 5,
            off_after_minutes: 10,
            off_enabled: true,
        }),
        num_lock_inverted: false,
        release_rebound_filter: false,
    },
    lightbar: openwoot_protocol::lightbar::Entries::new(),
    face: Face {
        mode: Mode::NoGamepad,
        keyboard: KeyboardFormat::NKeyRollover,
        speed: Speed::High,
        gamepad_polling: PollingRate::Every1Millisecond,
    },
};
static STATE: State<Mutual, (), ROWS, COLUMNS, PAGES, LENGTH> = State::new();

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Initialize,
    Running(bool),
    Write(std::vec::Vec<u8>),
}

#[derive(Default)]
struct Leds {
    calls: std::vec::Vec<Call>,
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the recording bus completes every call at once"
)]
impl LedMatrix for Leds {
    type Error = ();

    async fn initialize(&mut self) -> Result<(), ()> {
        self.calls.push(Call::Initialize);
        Ok(())
    }

    async fn set_running(&mut self, running: bool) -> Result<(), ()> {
        self.calls.push(Call::Running(running));
        Ok(())
    }

    async fn write(&mut self, _bank: u8, _first_register: u8, values: &[u8]) -> Result<(), ()> {
        self.calls.push(Call::Write(values.to_vec()));
        Ok(())
    }
}

impl Leds {
    fn lit_values(&self) -> usize {
        self.calls
            .iter()
            .filter_map(|call| match call {
                Call::Write(values) => Some(values.iter().filter(|value| **value != 0).count()),
                _ => None,
            })
            .sum()
    }

    fn writes(&self) -> usize {
        self.calls
            .iter()
            .filter(|call| matches!(call, Call::Write(_)))
            .count()
    }
}

fn storage() -> Settings<Nor> {
    let layout = NorLayout {
        capacity: 6 * SECTOR,
        settings: Region {
            start: 0,
            end: 4 * SECTOR,
        },
        device_info: None,
    };
    let mut settings = Settings::new();
    let opened =
        block_on(settings.open(Partition::new(Nor::new(6), layout.settings_region()), false));
    assert!(opened.is_ok(), "{opened:?}");
    settings
}

fn open<'a>(
    keys: &'a keys::Shared<Mutual>,
    shared: &'a profile::Shared<Mutual>,
    storage: Option<&'a mut Settings<Nor>>,
) -> ProfileModel<'a> {
    let mut model = Model::new(keys, shared, storage, &FACTORY, 0, 4);
    block_on(model.load());
    model
}

#[test]
fn factory_profiles_light_the_leds_once_the_host_configures_the_keyboard() {
    let model = open(STATE.keys(), STATE.profile(), None);
    assert_eq!(model.summary().count, 2);
    let host = Host::default();
    let Firmware {
        mut device,
        lighting,
        ..
    } = Firmware::new(
        &STATE,
        host.driver(),
        || Some(BusSpeed::High),
        FACTORY.face,
        IDENTITY,
        Lighting::new(&MAP, &VARIANT, GEOMETRY, (), 0),
    );
    let mut leds = Leds::default();
    block_on(lighting.step(&mut leds, &(), 0, lighting.host.is_active()));
    assert_eq!(leds.calls, []);
    let mut device = pin!(device.run());
    host.attach(device.as_mut());
    assert_eq!(
        host.control_out(device.as_mut(), setup(0, SET_CONFIGURATION, 1, 0, 0), &[]),
        Some(Outcome::Accepted)
    );
    assert!(poll(device.as_mut()).is_none());
    assert!(lighting.host.is_active());
    for now_ms in [20, 40, 300] {
        block_on(lighting.step(&mut leds, &(), now_ms, lighting.host.is_active()));
    }
    assert_eq!(leds.calls.first(), Some(&Call::Running(true)));
    assert!(leds.writes() >= PAGES);
    assert!(
        leds.lit_values() > PAGES,
        "{} lit values",
        leds.lit_values()
    );
    let lighting = block_on(lighting.lighting.lock());
    assert_eq!(lighting.brightness(), 255);
    assert!(
        lighting
            .key_color(keys::Key::new(2, 2))
            .is_some_and(|color| color != BLACK)
    );
}

#[test]
fn a_fresh_keyboard_has_the_factory_profiles_with_their_names_and_lighting() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(model.summary().count, 2);
    assert_eq!(
        block_on(model.info(Slot::new(1))).map(|info| info.name),
        Ok("Rapid Profile".try_into().unwrap())
    );
    assert_eq!(
        block_on(model.lighting(Slot::new(1))).map(|settings| settings.brightness),
        Ok(128)
    );
    let mut colors = None;
    assert_eq!(
        block_on(model.key_colors(Slot::new(0), Layer::Base, &mut colors)),
        Ok(())
    );
    assert_eq!(colors, Some(BLUE_KEYS.clone()));
    assert_eq!(
        block_on(model.settings(Slot::new(1))).map(|settings| settings.rapid_trigger_enabled),
        Ok(true)
    );
    assert_eq!(
        block_on(model.info(Slot::new(2))),
        Err(Rejection::ProfileMissing(2))
    );
    assert_eq!(
        model.device_settings(),
        device::Settings::from(FACTORY.face)
    );
    assert_eq!(model.global_settings(), FACTORY.global);
}

#[test]
fn profiles_are_created_up_to_the_capacity_and_only_the_last_one_is_deleted() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(block_on(model.create()), Ok(Slot::new(2)));
    assert_eq!(block_on(model.create()), Ok(Slot::new(3)));
    assert_eq!(block_on(model.create()), Err(Rejection::ProfileLimit));
    assert_eq!(keys.profiles(), (4, 0));
    assert_eq!(
        block_on(model.delete(Slot::new(0))),
        Err(Rejection::ProfileProtected(0))
    );
    assert_eq!(
        block_on(model.delete(Slot::new(2))),
        Err(Rejection::ProfileProtected(2))
    );
    block_on(model.activate(Slot::new(3))).unwrap();
    block_on(model.delete(Slot::new(3))).unwrap();
    assert_eq!(model.summary().count, 3);
    assert_eq!(model.summary().active, Slot::new(2));
    assert_eq!(keys.profiles(), (3, 2));
}

#[test]
fn lighting_and_colours_apply_at_once_and_stay_after_save() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let _ = shared.take_changes();
    let dim = LightingSettings {
        brightness: 40,
        ..PRESETS[0].lighting.clone()
    };
    block_on(model.set_lighting(Slot::new(0), dim.clone())).unwrap();
    assert!(shared.take_changes().contains(Changes::LIGHTING));
    assert_eq!(
        shared.read(|appearance| appearance.settings.brightness),
        Some(40)
    );
    block_on(model.revert());
    assert_eq!(
        shared.read(|appearance| appearance.settings.brightness),
        Some(255)
    );
    let revision = block_on(model.revision(Slot::new(0))).unwrap();
    block_on(model.set_lighting(Slot::new(0), dim.clone())).unwrap();
    let red = Vec::from_array([from_rgb565(0xF800); ROWS * COLUMNS]);
    block_on(model.set_key_colors(Slot::new(0), Layer::One, Some(&red))).unwrap();
    block_on(model.save()).unwrap();
    assert_eq!(block_on(model.revision(Slot::new(0))), Ok(revision + 1));
    block_on(model.revert());
    assert_eq!(block_on(model.lighting(Slot::new(0))), Ok(dim));
    let mut colors = None;
    block_on(model.key_colors(Slot::new(0), Layer::One, &mut colors)).unwrap();
    assert_eq!(colors, Some(red));
    let named = Info {
        name: "Mine".try_into().unwrap(),
        color: BLUE,
    };
    block_on(model.set_info(Slot::new(1), named.clone())).unwrap();
    assert_eq!(block_on(model.info(Slot::new(1))), Ok(named));
}

#[test]
fn a_profile_switch_saves_the_changes_of_the_profile_it_leaves() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    block_on(model.set_lighting(
        Slot::new(0),
        LightingSettings {
            brightness: 7,
            ..PRESETS[0].lighting.clone()
        },
    ))
    .unwrap();
    block_on(model.activate(Slot::new(1))).unwrap();
    assert_eq!(
        shared.read(|appearance| appearance.settings.brightness),
        Some(128)
    );
    assert_eq!(
        keys.read(|profile| profile.settings.rapid_trigger_enabled),
        Some(true)
    );
    block_on(model.activate(Slot::new(0))).unwrap();
    assert_eq!(
        shared.read(|appearance| appearance.settings.brightness),
        Some(7)
    );
}

#[test]
fn global_settings_reach_the_lighting_and_the_scan_at_once() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let _ = shared.take_changes();
    let settings = global::Settings {
        idle: None,
        num_lock_inverted: true,
        release_rebound_filter: true,
    };
    model.set_global_settings(settings);
    assert!(shared.take_changes().contains(Changes::GLOBAL));
    assert!(keys.rebound_filter());
    let device = device::Settings {
        mode: device::Mode::Xinput,
        ..device::Settings::from(FACTORY.face)
    };
    model.set_device_settings(device);
    block_on(model.save()).unwrap();
    block_on(model.revert());
    assert_eq!(model.global_settings(), settings);
    assert_eq!(model.device_settings(), device);
}

#[test]
fn brightness_keys_store_the_level_in_the_active_profile() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    shared.save_brightness(90);
    assert_eq!(block_on(shared.brightness_saved()), 90);
    block_on(model.save_brightness()).unwrap();
    block_on(model.revert());
    assert_eq!(
        block_on(model.lighting(Slot::new(0))).map(|settings| settings.brightness),
        Ok(90)
    );
}

#[test]
fn single_layers_of_an_inactive_profile_change_without_touching_the_others() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let inactive = Slot::new(1);
    let red = Vec::from_array([from_rgb565(0xF800); ROWS * COLUMNS]);
    block_on(model.set_key_colors(inactive, Layer::Two, Some(&red))).unwrap();
    let mut colors = None;
    block_on(model.key_colors(inactive, Layer::Two, &mut colors)).unwrap();
    assert_eq!(colors, Some(red));
    block_on(model.key_colors(inactive, Layer::Base, &mut colors)).unwrap();
    assert_eq!(colors, None);
    let typed = Vec::from_array([Action::Keyboard(0x05); ROWS * COLUMNS]);
    block_on(model.set_keymap(inactive, Layer::One, &typed)).unwrap();
    let mut keymap = Keymap::new();
    block_on(model.keymap(inactive, Layer::One, &mut keymap)).unwrap();
    assert_eq!(keymap, typed);
    block_on(model.keymap(inactive, Layer::Base, &mut keymap)).unwrap();
    assert_eq!(keymap.as_slice(), LAYERS[0].as_flattened());
    block_on(model.activate(inactive)).unwrap();
    assert_eq!(
        keys.read(|profile| profile.keymaps.0[1].clone()),
        Some(typed)
    );
    assert_eq!(
        shared.read(|appearance| appearance.colors.0[2].is_some()),
        Some(true)
    );
}
