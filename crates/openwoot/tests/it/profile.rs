use std::{pin::pin, rc::Rc};

use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_usb_driver::Speed as BusSpeed;
use openwoot::{
    analog::{self, SwitchModel},
    keys::{self, Key, ProfileSource},
    layout::{DefaultKeymap, Patch},
    lighting::{LedMatrix, Lighting, from_rgb565},
    profile::{self, Changes, Configurator, Factory, Model, Order, Preset},
    protocol::Configuration,
    runtime::{Firmware, State},
    storage::{LayoutOverride, NorLayout, Partition, Record, RecordKey, Scope, Settings},
    usb::{Face, KeyboardFormat, Mode, PollingRate, Speed},
};
use openwoot_image::Region;
use openwoot_protocol::{
    advanced,
    configuration::{Capabilities, Invalid, Rejection},
    device, gamepad, geometry, global,
    heapless::Vec,
    keys::{Action, Distance, Function, Index, Keymap, Layer, RapidTrigger, ScanMode, TapDuration},
    lighting::{
        Color, Effect, HueRange, Idle, Indicators, KeyColors, RainbowWave, ReactiveColoring,
        ReactiveFade, Rgb, Settings as LightingSettings,
    },
    profile::{Info, Slot, Summary},
    switches,
};

use crate::{
    lighting::{COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT},
    storage::{Nor, Probe, SECTOR, Watch},
    support::{Host, IDENTITY, Outcome, poll, setup},
};

type Mutual = CriticalSectionRawMutex;
pub(crate) type ProfileModel<'a> = Model<'a, Mutual, Nor, ROWS, COLUMNS>;
type ProbedModel<'a> = Model<'a, Mutual, Probe, ROWS, COLUMNS>;

const LAYOUT: NorLayout = NorLayout {
    capacity: 6 * SECTOR,
    settings: Region {
        start: 0,
        end: 4 * SECTOR,
    },
    device_info: None,
};

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
const FN_LAYERS: [[[Action; COLUMNS]; ROWS]; 2] = [
    [[Action::Keyboard(0x04); COLUMNS]; ROWS],
    [[Action::Keyboard(0x3A); COLUMNS]; ROWS],
];
const ISO_PATCHES: [Patch; 2] = [
    Patch {
        layer: Layer::Base,
        key: Key::new(3, 1),
        action: Action::Keyboard(0x64),
    },
    Patch {
        layer: Layer::One,
        key: Key::new(0, 0),
        action: Action::Function(Function::BrightnessUp),
    },
];
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
pub(crate) static FACTORY: Factory<ROWS, COLUMNS> = factory(DefaultKeymap {
    layers: &LAYERS,
    variant_patches: &[],
});
static LAYERED: Factory<ROWS, COLUMNS> = factory(DefaultKeymap {
    layers: &FN_LAYERS,
    variant_patches: &[&[], &ISO_PATCHES],
});
static STATE: State<Mutual, (), ROWS, COLUMNS, PAGES, LENGTH> = State::new();

const fn factory(keymap: DefaultKeymap<ROWS, COLUMNS>) -> Factory<ROWS, COLUMNS> {
    Factory {
        keymap,
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
    }
}

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

pub(crate) fn storage() -> Settings<Nor> {
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

fn storage_on(nor: Nor) -> Settings<Nor> {
    let mut settings = Settings::new();
    let opened = block_on(settings.open(Partition::new(nor, LAYOUT.settings_region()), false));
    assert!(opened.is_ok(), "{opened:?}");
    settings
}

fn probed_storage() -> (Settings<Probe>, Rc<Watch>) {
    let (probe, watch) = Probe::new(Nor::new(6));
    let mut settings = Settings::new();
    let opened = block_on(settings.open(Partition::new(probe, LAYOUT.settings_region()), false));
    assert!(opened.is_ok(), "{opened:?}");
    (settings, watch)
}

fn open_probed<'a>(
    keys: &'a keys::Shared<Mutual>,
    shared: &'a profile::Shared<Mutual>,
    storage: &'a mut Settings<Probe>,
) -> ProbedModel<'a> {
    let mut model = Model::new(keys, shared, Some(storage), &FACTORY, 0, 4);
    block_on(model.load());
    model
}

fn typed(usage: u8) -> Keymap {
    Vec::from_array([Action::Keyboard(usage); ROWS * COLUMNS])
}

pub(crate) fn open<'a>(
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
            .key_color(Key::new(2, 2))
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
    assert_eq!(
        block_on(model.create()),
        Err(Rejection::ProfileLimitReached)
    );
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
        icon: 7,
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

#[test]
fn a_profile_switch_reads_only_the_parts_the_profile_has_stored() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let (mut settings, watch) = probed_storage();
    let mut model = open_probed(&keys, &shared, &mut settings);
    for usage in 4..44 {
        block_on(model.set_keymap(Slot::new(0), Layer::One, &typed(usage))).unwrap();
        block_on(model.save()).unwrap();
    }
    let before = watch.reads();
    block_on(model.activate(Slot::new(1))).unwrap();
    let reads = watch.reads() - before;
    assert!(reads < 200, "{reads} reads");
    block_on(model.activate(Slot::new(0))).unwrap();
    assert_eq!(
        keys.read(|profile| profile.keymaps.0[1].clone()),
        Some(typed(43))
    );
}

#[test]
fn a_profile_count_that_cannot_be_stored_creates_and_deletes_nothing() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let (mut settings, watch) = probed_storage();
    let mut model = open_probed(&keys, &shared, &mut settings);
    watch.fail_writes(true);
    assert_eq!(
        block_on(model.create()),
        Err(Rejection::StorageInaccessible)
    );
    assert_eq!(model.summary().count, 2);
    assert_eq!(keys.profiles(), (2, 0));
    watch.fail_writes(false);
    assert_eq!(block_on(model.create()), Ok(Slot::new(2)));
    block_on(model.activate(Slot::new(2))).unwrap();
    watch.fail_writes(true);
    assert_eq!(
        block_on(model.delete(Slot::new(2))),
        Err(Rejection::StorageInaccessible)
    );
    assert_eq!(
        model.summary(),
        Summary {
            count: 3,
            active: Slot::new(2)
        }
    );
    assert_eq!(keys.profiles(), (3, 2));
    assert_eq!(
        block_on(model.lighting(Slot::new(2))).map(|settings| settings.brightness),
        Ok(255)
    );
    watch.fail_writes(false);
    block_on(model.delete(Slot::new(2))).unwrap();
    assert_eq!(
        model.summary(),
        Summary {
            count: 2,
            active: Slot::new(1)
        }
    );
}

#[test]
fn a_power_cut_while_a_profile_is_deleted_leaves_it_whole_or_gone() {
    let named = Info {
        name: "Mine".try_into().unwrap(),
        color: BLUE,
        icon: 5,
    };
    let dim = LightingSettings {
        brightness: 40,
        indicators: INDICATORS,
        effects: Vec::new(),
    };
    let base = {
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(Nor::new(6));
        {
            let mut model = open(&keys, &shared, Some(&mut settings));
            assert_eq!(block_on(model.create()), Ok(Slot::new(2)));
            block_on(model.set_keymap(Slot::new(2), Layer::One, &typed(0x05))).unwrap();
            block_on(model.set_lighting(Slot::new(2), dim.clone())).unwrap();
            block_on(model.set_info(Slot::new(2), named.clone())).unwrap();
        }
        settings.release().map(Partition::release).unwrap()
    };
    for cut in 0.. {
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(base.cut_after(cut));
        let deleted = block_on(open(&keys, &shared, Some(&mut settings)).delete(Slot::new(2)));
        let nor = settings.release().map(Partition::release).unwrap();
        if !nor.was_cut() {
            deleted.unwrap();
            break;
        }
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(nor.revived());
        let mut model = open(&keys, &shared, Some(&mut settings));
        if model.summary().count == 2 {
            continue;
        }
        assert_eq!(model.summary().count, 3, "cut {cut}");
        let mut keymap = Keymap::new();
        block_on(model.keymap(Slot::new(2), Layer::One, &mut keymap)).unwrap();
        assert_eq!(keymap, typed(0x05), "cut {cut}");
        assert_eq!(
            block_on(model.lighting(Slot::new(2))),
            Ok(dim.clone()),
            "cut {cut}"
        );
        assert_eq!(
            block_on(model.info(Slot::new(2))),
            Ok(named.clone()),
            "cut {cut}"
        );
    }
}

#[test]
fn a_brightness_key_stores_the_level_without_the_unsaved_lighting() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    let unsaved = LightingSettings {
        brightness: 200,
        indicators: INDICATORS,
        effects: Vec::new(),
    };
    block_on(model.set_lighting(Slot::new(0), unsaved)).unwrap();
    shared.save_brightness(90);
    assert_eq!(block_on(shared.brightness_saved()), 90);
    let revision = block_on(model.revision(Slot::new(0))).unwrap();
    block_on(model.save_brightness()).unwrap();
    assert_eq!(block_on(model.revision(Slot::new(0))), Ok(revision + 1));
    block_on(model.save_brightness()).unwrap();
    assert_eq!(block_on(model.revision(Slot::new(0))), Ok(revision + 1));
    block_on(model.revert());
    assert_eq!(
        block_on(model.lighting(Slot::new(0))),
        Ok(LightingSettings {
            brightness: 90,
            ..PRESETS[0].lighting.clone()
        })
    );
}

#[test]
fn a_profile_switch_stores_a_pending_brightness_level_for_the_profile_it_leaves() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    shared.save_brightness(90);
    block_on(model.activate(Slot::new(1))).unwrap();
    assert!(!shared.take_brightness_save());
    assert_eq!(
        block_on(model.lighting(Slot::new(1))).map(|settings| settings.brightness),
        Ok(128)
    );
    assert_eq!(
        block_on(model.lighting(Slot::new(0))).map(|settings| settings.brightness),
        Ok(90)
    );
}

#[test]
fn a_stored_colour_layer_that_does_not_cover_the_key_matrix_gives_way_to_its_factory_layer() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let red: KeyColors = Vec::from_array([from_rgb565(0xF800); ROWS * COLUMNS]);
    let short: KeyColors = Vec::from_array([BLUE, BLACK]);
    let stored = profile::Colors([Some(red.clone()), Some(short), None, None]);
    block_on(settings.store(Scope::Profile(0), &stored)).unwrap();
    block_on(settings.store(Scope::Profile(1), &stored)).unwrap();
    let mut model = open(&keys, &shared, Some(&mut settings));
    assert_eq!(
        shared.read(|appearance| appearance.colors.clone()),
        Some(profile::Colors([Some(red.clone()), None, None, None]))
    );
    let mut colors = Some(KeyColors::new());
    block_on(model.key_colors(Slot::new(1), Layer::One, &mut colors)).unwrap();
    assert_eq!(colors, None);
    block_on(model.key_colors(Slot::new(1), Layer::Base, &mut colors)).unwrap();
    assert_eq!(colors.as_ref(), Some(&red));
    block_on(model.set_key_colors(Slot::new(1), Layer::Two, Some(&red))).unwrap();
    block_on(model.activate(Slot::new(1))).unwrap();
    assert_eq!(
        shared.read(|appearance| appearance.colors.clone()),
        Some(profile::Colors([Some(red.clone()), None, Some(red), None]))
    );
}

#[test]
fn stored_parts_that_fail_validation_give_way_to_the_factory_parts() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let stray: advanced::Keys = Vec::from_array([advanced::Key {
        key: Index::new(200),
        layer: Layer::Base,
        behaviour: advanced::Behaviour::Toggle(Action::Keyboard(0x04)),
    }]);
    let flat = openwoot_protocol::keys::Settings {
        actuation: Distance::from_fraction(0),
        ..TYPING
    };
    let biased = gamepad::Settings {
        left_stick_up_bias: Some(150),
        ..keys::default_gamepad_settings()
    };
    let frozen = LightingSettings {
        brightness: 9,
        indicators: INDICATORS,
        effects: Vec::from_array([Effect::RainbowWave(RainbowWave {
            hues: HueRange::From113To204,
            period: 0,
            forward: false,
        })]),
    };
    let mut keymaps = keys::Keymaps::default();
    keymaps.0[1] = typed(0x05);
    let mut refused = typed(0x06);
    refused[0] = Action::Layer(Layer::Base);
    keymaps.0[2] = refused;
    for slot in [0, 1] {
        let scope = Scope::Profile(slot);
        block_on(settings.store(scope, &stray)).unwrap();
        block_on(settings.store(scope, &flat)).unwrap();
        block_on(settings.store(scope, &biased)).unwrap();
        block_on(settings.store(scope, &frozen)).unwrap();
        block_on(settings.store(scope, &keymaps)).unwrap();
    }
    let mut model = open(&keys, &shared, Some(&mut settings));
    let factory_layer = |layer: Layer| FACTORY.keymap.actions(layer, 0).collect::<Keymap>();
    assert_eq!(keys.read(|profile| profile.advanced.len()), Some(0));
    assert_eq!(keys.read(|profile| profile.settings), Some(TYPING));
    assert_eq!(
        keys.read(|profile| profile.gamepad_settings),
        Some(keys::default_gamepad_settings())
    );
    assert_eq!(
        keys.read(|profile| profile.keymaps.clone()),
        Some(keys::Keymaps([
            factory_layer(Layer::Base),
            typed(0x05),
            factory_layer(Layer::Two),
            factory_layer(Layer::Three),
        ]))
    );
    assert_eq!(
        shared.read(|appearance| appearance.settings.clone()),
        Some(PRESETS[0].lighting.clone())
    );
    let inactive = Slot::new(1);
    let mut advanced = advanced::Keys::new();
    block_on(model.advanced(inactive, &mut advanced)).unwrap();
    assert!(advanced.is_empty());
    assert_eq!(block_on(model.settings(inactive)), Ok(PRESETS[1].keys));
    assert_eq!(
        block_on(model.gamepad_settings(inactive)),
        Ok(keys::default_gamepad_settings())
    );
    assert_eq!(
        block_on(model.lighting(inactive)),
        Ok(PRESETS[1].lighting.clone())
    );
    let mut keymap = Keymap::new();
    block_on(model.keymap(inactive, Layer::One, &mut keymap)).unwrap();
    assert_eq!(keymap, typed(0x05));
    block_on(model.keymap(inactive, Layer::Two, &mut keymap)).unwrap();
    assert_eq!(keymap, factory_layer(Layer::Two));
}

#[test]
fn the_layout_is_the_one_of_the_layout_variant_of_the_unit() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let capabilities = Capabilities {
        key_rows: u8::try_from(ROWS).unwrap(),
        key_columns: u8::try_from(COLUMNS).unwrap(),
        profiles: 4,
        layouts: Vec::from_array([
            device::Layout::Ansi,
            device::Layout::Iso,
            device::Layout::Jis,
        ]),
        lightbar_leds: 0,
        host_signals: false,
        switch_models: 1,
        edition: None,
    };
    for (variant, layout) in [
        (0, device::Layout::Ansi),
        (1, device::Layout::Iso),
        (2, device::Layout::Jis),
        (9, device::Layout::Ansi),
    ] {
        let model: Mutex<Mutual, ProfileModel<'_>> =
            Mutex::new(Model::new(&keys, &shared, None, &FACTORY, variant, 4));
        let mut configurator = Configurator::new(&model, capabilities.clone());
        assert_eq!(
            block_on(configurator.layout()),
            Ok(device::LayoutState {
                active: layout,
                chosen: layout
            }),
            "variant {variant}"
        );
    }
}

#[test]
fn the_key_geometry_is_the_one_of_the_layout_in_effect() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let tables = [100, 150, 200].map(|width| {
        let mut table = [[None; COLUMNS]; ROWS];
        table[1][2] = Some(geometry::Key {
            x: 200,
            y: 100,
            width,
            height: 100,
            shape: geometry::Shape::Rectangle,
        });
        table
    });
    for (variant, width) in [(0, 100), (1, 150), (2, 200), (9, 100)] {
        let model: Mutex<Mutual, ProfileModel<'_>> =
            Mutex::new(Model::new(&keys, &shared, None, &FACTORY, variant, 4));
        let mut configurator =
            Configurator::new(&model, switch_capabilities()).with_geometry(&tables);
        let mut served = geometry::Keys::new();
        assert_eq!(block_on(configurator.key_geometry(&mut served)), Ok(()));
        assert_eq!(served.len(), ROWS * COLUMNS);
        assert_eq!(
            served[COLUMNS + 2].map(|key| key.width),
            Some(width),
            "variant {variant}"
        );
        assert_eq!(served.iter().flatten().count(), 1);
    }
    let model: Mutex<Mutual, ProfileModel<'_>> =
        Mutex::new(Model::new(&keys, &shared, None, &FACTORY, 0, 4));
    let mut without = Configurator::new(&model, switch_capabilities());
    assert_eq!(
        block_on(without.key_geometry(&mut geometry::Keys::new())),
        Err(Rejection::Unsupported)
    );
}

pub(crate) fn switch_capabilities() -> Capabilities {
    Capabilities {
        key_rows: u8::try_from(ROWS).unwrap_or(u8::MAX),
        key_columns: u8::try_from(COLUMNS).unwrap_or(u8::MAX),
        profiles: 4,
        layouts: Vec::from_array([device::Layout::Ansi, device::Layout::Iso]),
        lightbar_leds: 0,
        host_signals: false,
        switch_models: SwitchModel::COUNT,
        edition: None,
    }
}

pub(crate) const fn chosen(model: u8, travel_offset: u8) -> switches::Switch {
    switches::Switch {
        model,
        travel_offset,
    }
}

pub(crate) fn factory_switches() -> switches::Settings {
    switches::Settings {
        default: None,
        keys: Vec::from_array([None; ROWS * COLUMNS]),
    }
}

fn mixed_switches() -> switches::Settings {
    let mut settings = factory_switches();
    settings.default = Some(chosen(2, 0));
    if let Some(key) = settings.keys.get_mut(1) {
        *key = Some(chosen(0, 20));
    }
    settings
}

#[test]
fn a_fresh_keyboard_has_the_factory_switch_on_every_key_and_lists_the_known_models() {
    let kind = |id| SwitchModel::new(id).unwrap();
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = Configurator::new(&model, switch_capabilities());
    assert_eq!(block_on(configurator.switches()), Ok(factory_switches()));
    let models = block_on(configurator.switch_models()).unwrap();
    assert_eq!(models.len(), usize::from(SwitchModel::COUNT));
    assert_eq!(
        models.first(),
        Some(&switches::Model {
            travel: 80,
            group: 0
        })
    );
    assert_eq!(
        models.get(3),
        Some(&switches::Model {
            travel: 68,
            group: 7
        })
    );
    let mut narrow = Configurator::new(
        &model,
        Capabilities {
            switch_models: 3,
            ..switch_capabilities()
        },
    );
    assert_eq!(
        block_on(narrow.switch_models()).map(|models| models.len()),
        Ok(3)
    );
    for id in 0..SwitchModel::COUNT {
        assert_eq!(
            keys.fitted_switch(usize::from(id), kind(id)),
            analog::Switch::new(kind(id), None)
        );
    }
}

#[test]
fn switches_reach_the_keys_at_once_are_stored_by_save_and_dropped_by_revert() {
    let kind = |id| SwitchModel::new(id).unwrap();
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    {
        let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
        let mut configurator = Configurator::new(&model, switch_capabilities());
        let _ = keys.take_switch_change();
        block_on(configurator.set_switches(mixed_switches())).unwrap();
        assert!(keys.take_switch_change());
        assert_eq!(block_on(configurator.switches()), Ok(mixed_switches()));
        assert_eq!(
            keys.fitted_switch(0, SwitchModel::DEFAULT),
            analog::Switch::new(kind(2), None)
        );
        assert_eq!(
            keys.fitted_switch(1, SwitchModel::DEFAULT),
            analog::Switch::new(kind(0), Some(20))
        );
        block_on(configurator.revert()).unwrap();
        assert_eq!(block_on(configurator.switches()), Ok(factory_switches()));
        assert!(keys.take_switch_change());
        assert_eq!(
            keys.fitted_switch(1, kind(1)),
            analog::Switch::new(kind(1), None)
        );
        block_on(configurator.revert()).unwrap();
        assert!(!keys.take_switch_change());
        block_on(configurator.set_switches(mixed_switches())).unwrap();
        block_on(configurator.save()).unwrap();
        assert_eq!(block_on(configurator.profile_revision(Slot::new(0))), Ok(0));
        block_on(configurator.revert()).unwrap();
        assert_eq!(block_on(configurator.switches()), Ok(mixed_switches()));
    }
    assert_eq!(
        block_on(settings.load::<switches::Settings>(Scope::Global)),
        Ok(Some(mixed_switches()))
    );
    let restarted = keys::Shared::new();
    let model = open(&restarted, &shared, Some(&mut settings));
    assert_eq!(model.switches(), mixed_switches());
    assert!(restarted.take_switch_change());
    assert_eq!(
        restarted.fitted_switch(1, SwitchModel::DEFAULT),
        analog::Switch::new(kind(0), Some(20))
    );
}

#[test]
fn switches_the_keyboard_cannot_fit_are_refused_and_change_nothing() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let model = Mutex::new(open(&keys, &shared, None));
    let mut configurator = Configurator::new(&model, switch_capabilities());
    let mut longest = factory_switches();
    longest.keys[0] = Some(chosen(0, 79));
    longest.keys[1] = Some(chosen(2, 65));
    assert_eq!(block_on(configurator.set_switches(longest.clone())), Ok(()));
    let _ = keys.take_switch_change();
    for (model, offset, travel) in [(0, 80, 80), (0, 255, 80), (2, 66, 66)] {
        let mut beyond = longest.clone();
        beyond.keys[2] = Some(chosen(model, offset));
        assert_eq!(
            block_on(configurator.set_switches(beyond)),
            Err(Rejection::Invalid(Invalid::TravelOffsetReachesTravel {
                model,
                offset,
                travel
            }))
        );
    }
    let default_beyond = switches::Settings {
        default: Some(chosen(0, 80)),
        ..longest.clone()
    };
    assert_eq!(
        block_on(configurator.set_switches(default_beyond)),
        Err(Rejection::Invalid(Invalid::TravelOffsetReachesTravel {
            model: 0,
            offset: 80,
            travel: 80
        }))
    );
    let mut unknown = longest.clone();
    unknown.keys[2] = Some(chosen(SwitchModel::COUNT, 0));
    assert_eq!(
        block_on(configurator.set_switches(unknown)),
        Err(Rejection::Invalid(Invalid::SwitchModelUnknown(
            SwitchModel::COUNT
        )))
    );
    let mut many = factory_switches();
    many.default = Some(chosen(1, 17));
    for (offset, key) in (1..).zip(many.keys.iter_mut()) {
        *key = Some(chosen(1, offset));
    }
    assert_eq!(
        block_on(configurator.set_switches(many)),
        Err(Rejection::Invalid(Invalid::TunedSwitchesExceedLimit))
    );
    assert!(!keys.take_switch_change());
    assert_eq!(block_on(configurator.switches()), Ok(longest));
    assert_eq!(
        block_on(configurator.save()),
        Err(Rejection::StorageInaccessible)
    );
}

#[test]
fn a_stored_switch_setting_that_fails_validation_gives_way_to_the_factory_switches() {
    let kind = |id| SwitchModel::new(id).unwrap();
    let shared = profile::Shared::new();
    let mut beyond = mixed_switches();
    beyond.keys[2] = Some(chosen(0, 80));
    let mut short = mixed_switches();
    short.keys.pop();
    let mut unknown = mixed_switches();
    unknown.default = Some(chosen(SwitchModel::COUNT, 0));
    for stored in [beyond, short, unknown] {
        let keys = keys::Shared::new();
        let mut settings = storage();
        block_on(settings.store(Scope::Global, &stored)).unwrap();
        let model = open(&keys, &shared, Some(&mut settings));
        assert_eq!(model.switches(), factory_switches(), "{stored:?}");
        assert_eq!(
            keys.fitted_switch(2, kind(1)),
            analog::Switch::new(kind(1), None)
        );
    }
}

#[test]
fn the_chosen_layout_is_stored_by_save_and_in_effect_from_the_next_start() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    {
        let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
        let mut configurator = Configurator::new(&model, switch_capabilities());
        assert_eq!(
            block_on(configurator.set_layout(device::Layout::Iso)),
            Ok(())
        );
        assert_eq!(
            block_on(configurator.layout()),
            Ok(layouts(device::Layout::Ansi, device::Layout::Iso))
        );
        block_on(configurator.revert()).unwrap();
        assert_eq!(
            block_on(configurator.layout()),
            Ok(layouts(device::Layout::Ansi, device::Layout::Ansi))
        );
        block_on(configurator.save()).unwrap();
    }
    assert_eq!(
        block_on(settings.load::<LayoutOverride>(Scope::Global)),
        Ok(None)
    );
    {
        let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
        let mut configurator = Configurator::new(&model, switch_capabilities());
        block_on(configurator.set_layout(device::Layout::Iso)).unwrap();
        block_on(configurator.save()).unwrap();
        assert_eq!(block_on(configurator.profile_revision(Slot::new(0))), Ok(0));
        block_on(configurator.revert()).unwrap();
        assert_eq!(
            block_on(configurator.layout()),
            Ok(layouts(device::Layout::Ansi, device::Layout::Iso))
        );
    }
    let variant = block_on(LayoutOverride::variant_or(&mut settings, 0));
    assert_eq!(variant, 1);
    let mut restarted: ProfileModel<'_> =
        Model::new(&keys, &shared, Some(&mut settings), &FACTORY, variant, 4)
            .with_factory_variant(0);
    block_on(restarted.load());
    let model = Mutex::new(restarted);
    let mut configurator = Configurator::new(&model, switch_capabilities());
    assert_eq!(
        block_on(configurator.layout()),
        Ok(layouts(device::Layout::Iso, device::Layout::Iso))
    );
    block_on(configurator.set_layout(device::Layout::Ansi)).unwrap();
    assert_eq!(
        block_on(configurator.layout()),
        Ok(layouts(device::Layout::Iso, device::Layout::Ansi))
    );
    block_on(configurator.revert()).unwrap();
    assert_eq!(
        block_on(configurator.layout()),
        Ok(layouts(device::Layout::Iso, device::Layout::Iso))
    );
    block_on(configurator.factory_reset()).unwrap();
    assert_eq!(
        block_on(configurator.layout()),
        Ok(layouts(device::Layout::Iso, device::Layout::Ansi))
    );
}

pub(crate) const fn layouts(active: device::Layout, chosen: device::Layout) -> device::LayoutState {
    device::LayoutState { active, chosen }
}

#[test]
fn a_layout_the_keyboard_cannot_have_is_refused() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let model = Mutex::new(open(&keys, &shared, None));
    let mut configurator = Configurator::new(&model, switch_capabilities());
    assert_eq!(
        block_on(configurator.set_layout(device::Layout::Jis)),
        Err(Rejection::Invalid(Invalid::LayoutMissing(
            device::Layout::Jis
        )))
    );
    assert_eq!(block_on(configurator.save()), Ok(()));
}

#[test]
fn a_stored_layout_outside_the_known_layouts_gives_way_to_the_factory_layout() {
    let mut settings = storage();
    assert_eq!(block_on(LayoutOverride::variant_or(&mut settings, 1)), 1);
    block_on(settings.store(Scope::Global, &LayoutOverride(2))).unwrap();
    assert_eq!(block_on(LayoutOverride::variant_or(&mut settings, 1)), 2);
    block_on(settings.store(Scope::Global, &LayoutOverride(3))).unwrap();
    assert_eq!(block_on(LayoutOverride::variant_or(&mut settings, 1)), 1);
}

#[test]
fn a_factory_reset_returns_the_switches_and_the_layout_to_the_factory() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    {
        let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
        let mut configurator = Configurator::new(&model, switch_capabilities());
        block_on(configurator.set_switches(mixed_switches())).unwrap();
        block_on(configurator.set_layout(device::Layout::Iso)).unwrap();
        block_on(configurator.save()).unwrap();
        let _ = keys.take_switch_change();
        block_on(configurator.factory_reset()).unwrap();
        assert_eq!(block_on(configurator.switches()), Ok(factory_switches()));
        assert!(keys.take_switch_change());
        assert_eq!(
            block_on(configurator.layout()),
            Ok(layouts(device::Layout::Ansi, device::Layout::Ansi))
        );
    }
    assert_eq!(block_on(LayoutOverride::variant_or(&mut settings, 0)), 0);
}

#[test]
fn the_default_keymap_is_the_factory_keymap_of_the_layout_in_effect_on_every_layer() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let key_count = ROWS * COLUMNS;
    let patched = |layer: [[Action; COLUMNS]; ROWS], patch: &Patch| {
        let mut layer = layer;
        layer[patch.key.row()][patch.key.column()] = patch.action;
        Keymap::from_slice(layer.as_flattened()).unwrap()
    };
    let plain =
        |layer: [[Action; COLUMNS]; ROWS]| Keymap::from_slice(layer.as_flattened()).unwrap();
    for (variant, base, function) in [
        (0, plain(FN_LAYERS[0]), plain(FN_LAYERS[1])),
        (
            1,
            patched(FN_LAYERS[0], &ISO_PATCHES[0]),
            patched(FN_LAYERS[1], &ISO_PATCHES[1]),
        ),
    ] {
        let model: Mutex<Mutual, ProfileModel<'_>> =
            Mutex::new(Model::new(&keys, &shared, None, &LAYERED, variant, 4));
        let mut configurator = Configurator::new(&model, switch_capabilities());
        let mut keymap = Keymap::new();
        for (layer, expected) in [
            (Layer::Base, base),
            (Layer::One, function),
            (Layer::Two, Vec::from_array([Action::None; ROWS * COLUMNS])),
        ] {
            block_on(configurator.default_keymap(layer, &mut keymap)).unwrap();
            assert_eq!(keymap, expected, "variant {variant}, {layer:?}");
            assert_eq!(keymap.len(), key_count);
            let mut unstored = Keymap::new();
            block_on(configurator.keymap(Slot::new(1), layer, &mut unstored)).unwrap();
            assert_eq!(unstored, expected, "variant {variant}, {layer:?}");
        }
    }
}

#[test]
fn a_profile_icon_stays_with_its_profile_and_is_saved_like_its_name() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let iconic = |icon| Info {
        name: "Aim".try_into().unwrap(),
        color: BLUE,
        icon,
    };
    {
        let mut model = open(&keys, &shared, Some(&mut settings));
        for slot in [0, 1] {
            assert_eq!(
                block_on(model.info(Slot::new(slot))).map(|info| info.icon),
                Ok(0)
            );
        }
        block_on(model.set_info(Slot::new(0), iconic(9))).unwrap();
        assert_eq!(block_on(model.info(Slot::new(0))), Ok(iconic(9)));
        block_on(model.revert());
        assert_eq!(
            block_on(model.info(Slot::new(0))).map(|info| info.icon),
            Ok(0)
        );
        block_on(model.set_info(Slot::new(0), iconic(9))).unwrap();
        block_on(model.save()).unwrap();
        block_on(model.set_info(Slot::new(1), iconic(u8::MAX))).unwrap();
    }
    let mut reopened = open(&keys, &shared, Some(&mut settings));
    assert_eq!(block_on(reopened.info(Slot::new(0))), Ok(iconic(9)));
    assert_eq!(block_on(reopened.info(Slot::new(1))), Ok(iconic(u8::MAX)));
}

fn names(model: &mut ProfileModel<'_>) -> std::vec::Vec<String> {
    (0..model.summary().count)
        .map(|slot| {
            block_on(model.info(Slot::new(slot))).map_or_else(
                |rejection| rejection.to_string(),
                |info| info.name.as_str().to_owned(),
            )
        })
        .collect()
}

fn named(name: &str) -> Info {
    let mut text = heapless::String::new();
    assert!(text.push_str(name).is_ok(), "`{name}` is too long");
    Info {
        name: text,
        color: BLUE,
        icon: 1,
    }
}

#[test]
fn swapped_profiles_keep_their_parts_and_the_active_profile_stays_active() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let dim = LightingSettings {
        brightness: 7,
        ..PRESETS[0].lighting.clone()
    };
    {
        let mut model = open(&keys, &shared, Some(&mut settings));
        block_on(model.set_keymap(Slot::new(1), Layer::One, &typed(0x05))).unwrap();
        block_on(model.set_lighting(Slot::new(0), dim.clone())).unwrap();
        let revision = block_on(model.revision(Slot::new(1))).unwrap();
        block_on(model.swap(Slot::new(0), Slot::new(1))).unwrap();
        assert_eq!(names(&mut model), ["Rapid Profile", "Typing Profile"]);
        assert_eq!(
            model.summary(),
            Summary {
                count: 2,
                active: Slot::new(1)
            }
        );
        assert_eq!(keys.profiles(), (2, 1));
        assert_eq!(block_on(model.revision(Slot::new(0))), Ok(revision));
        let mut keymap = Keymap::new();
        block_on(model.keymap(Slot::new(0), Layer::One, &mut keymap)).unwrap();
        assert_eq!(keymap, typed(0x05));
        assert_eq!(
            block_on(model.settings(Slot::new(0))).map(|settings| settings.rapid_trigger_enabled),
            Ok(true)
        );
        assert_eq!(
            block_on(model.lighting(Slot::new(0))).map(|settings| settings.brightness),
            Ok(128)
        );
        assert_eq!(block_on(model.lighting(Slot::new(1))), Ok(dim.clone()));
        block_on(model.save()).unwrap();
        block_on(model.swap(Slot::new(1), Slot::new(1))).unwrap();
    }
    let mut reopened = open(&keys, &shared, Some(&mut settings));
    assert_eq!(names(&mut reopened), ["Rapid Profile", "Typing Profile"]);
    assert_eq!(reopened.summary().active, Slot::new(1));
    assert_eq!(block_on(reopened.lighting(Slot::new(1))), Ok(dim));
    block_on(reopened.activate(Slot::new(0))).unwrap();
    assert_eq!(
        keys.read(|profile| profile.keymaps.0[1].clone()),
        Some(typed(0x05))
    );
}

#[test]
fn a_moved_profile_takes_its_new_slot_and_the_profiles_between_close_up() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let mut model = open(&keys, &shared, Some(&mut settings));
    for _ in 0..2 {
        block_on(model.create()).unwrap();
    }
    for (slot, name) in [(1, "B"), (2, "C"), (3, "D")] {
        block_on(model.set_info(Slot::new(slot), named(name))).unwrap();
    }
    block_on(model.shift(Slot::new(3), Slot::new(0))).unwrap();
    assert_eq!(names(&mut model), ["D", "Typing Profile", "B", "C"]);
    assert_eq!(model.summary().active, Slot::new(1));
    block_on(model.shift(Slot::new(1), Slot::new(3))).unwrap();
    assert_eq!(names(&mut model), ["D", "B", "C", "Typing Profile"]);
    assert_eq!(model.summary().active, Slot::new(3));
    assert_eq!(keys.profiles(), (4, 3));
    block_on(model.shift(Slot::new(2), Slot::new(2))).unwrap();
    assert_eq!(
        block_on(model.shift(Slot::new(0), Slot::new(4))),
        Err(Rejection::ProfileMissing(4))
    );
    assert_eq!(names(&mut model), ["D", "B", "C", "Typing Profile"]);
    block_on(model.delete(Slot::new(3))).unwrap();
    assert_eq!(names(&mut model), ["D", "B", "C"]);
    assert_eq!(model.summary().active, Slot::new(2));
    assert_eq!(block_on(model.create()), Ok(Slot::new(3)));
    assert_eq!(names(&mut model), ["D", "B", "C", "Typing Profile"]);
    assert_eq!(
        block_on(model.lighting(Slot::new(3))).map(|settings| settings.brightness),
        Ok(255)
    );
}

#[test]
fn a_stored_order_is_kept_only_while_it_places_every_profile_slot_once() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    for (stored, expected) in [
        (&[1_u8, 0, 2, 3][..], ["Rapid Profile", "Typing Profile"]),
        (&[1, 1, 2, 3][..], ["Typing Profile", "Rapid Profile"]),
        (&[1, 0][..], ["Typing Profile", "Rapid Profile"]),
        (&[1, 0, 2, 3, 4][..], ["Typing Profile", "Rapid Profile"]),
    ] {
        block_on(settings.store_with(
            RecordKey::of::<Order>(Scope::Global),
            <Order as Record>::VERSION,
            |body| postcard::to_slice(stored, body).ok().map(|body| body.len()),
        ))
        .unwrap();
        let mut model = open(&keys, &shared, Some(&mut settings));
        assert_eq!(names(&mut model), expected, "{stored:?}");
    }
}

#[test]
fn a_reorder_that_cannot_be_stored_changes_nothing() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let (mut settings, watch) = probed_storage();
    let mut model = open_probed(&keys, &shared, &mut settings);
    watch.fail_writes(true);
    assert_eq!(
        block_on(model.swap(Slot::new(0), Slot::new(1))),
        Err(Rejection::StorageInaccessible)
    );
    assert_eq!(
        block_on(model.shift(Slot::new(1), Slot::new(0))),
        Err(Rejection::StorageInaccessible)
    );
    assert_eq!(block_on(model.swap(Slot::new(1), Slot::new(1))), Ok(()));
    assert_eq!(model.summary().active, Slot::new(0));
    assert_eq!(keys.profiles(), (2, 0));
    assert_eq!(
        block_on(model.info(Slot::new(0))).map(|info| info.name),
        Ok("Typing Profile".try_into().unwrap())
    );
}

#[test]
fn a_power_cut_while_profiles_swap_leaves_them_in_one_order_or_the_other() {
    let base = {
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(Nor::new(6));
        {
            let mut model = open(&keys, &shared, Some(&mut settings));
            block_on(model.set_info(Slot::new(1), named("Mine"))).unwrap();
        }
        settings.release().map(Partition::release).unwrap()
    };
    for cut in 0.. {
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(base.cut_after(cut));
        let swapped =
            block_on(open(&keys, &shared, Some(&mut settings)).swap(Slot::new(0), Slot::new(1)));
        let nor = settings.release().map(Partition::release).unwrap();
        if !nor.was_cut() {
            swapped.unwrap();
            break;
        }
        let keys = keys::Shared::new();
        let shared = profile::Shared::new();
        let mut settings = storage_on(nor.revived());
        let mut model = open(&keys, &shared, Some(&mut settings));
        let order = names(&mut model);
        assert!(
            order == ["Typing Profile", "Mine"] || order == ["Mine", "Typing Profile"],
            "cut {cut}: {order:?}"
        );
        let active = model.summary().active;
        assert_eq!(
            block_on(model.info(active)).map(|info| info.name),
            Ok("Typing Profile".try_into().unwrap()),
            "cut {cut}"
        );
    }
}
