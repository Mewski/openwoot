use core::future::ready;

use embassy_futures::{block_on, select::select};
use openwoot::{
    keys::travel_of,
    layout::DefaultKeymap,
    lighting::Lighting,
    profile::{Configurator, Factory, Preset},
    protocol::Configuration,
    usb::{Face, Mode},
};
use openwoot_protocol::{
    heapless::Vec,
    keys::{Action, Distance, Function, RapidTrigger, ScanMode, TapDuration},
    lighting::{Effect, HueRange, KeyColors, RainbowWave, Rgb, Settings},
    profile::Slot,
};

use super::{
    pipeline::{Bench80, FULL, GLOBAL, INDICATORS, PROFILE_KEY, World, capabilities},
    support::{COLUMNS, KEY_A, ROWS},
};
use crate::lighting::{GEOMETRY, MAP, VARIANT};

const NOTHING: Action = Action::None;
const MODE_ROW: [Action; COLUMNS] = [
    NOTHING,
    Action::Keyboard(0x04),
    Action::Keyboard(0x16),
    Action::Keyboard(0x07),
    Action::Keyboard(0x09),
    Action::Keyboard(0x0A),
    Action::Function(Function::ToggleModeProfile),
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
    NOTHING,
];
const LAYERS: [[[Action; COLUMNS]; ROWS]; 1] = [[
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
    MODE_ROW,
    [NOTHING; COLUMNS],
    [NOTHING; COLUMNS],
]];
const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};
const RED: Rgb = Rgb {
    red: 255,
    green: 0,
    blue: 0,
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
const RAPID: openwoot_protocol::keys::Settings = openwoot_protocol::keys::Settings {
    actuation: Distance::from_fraction(26 * 257),
    rapid_trigger_enabled: true,
    rapid_trigger: RapidTrigger {
        press: Distance::from_fraction(26 * 257),
        release: None,
        continuous: false,
    },
    ..TYPING
};
static RED_KEYS: KeyColors = Vec::from_array([RED; ROWS * COLUMNS]);
static PRESETS: [Preset; 2] = [
    Preset {
        name: "Typing Profile",
        color: BLACK,
        keys: TYPING,
        lighting: Settings {
            brightness: 255,
            indicators: INDICATORS,
            effects: Vec::from_array([Effect::RainbowWave(RainbowWave {
                hues: HueRange::FullCircle,
                period: 8,
                forward: true,
            })]),
        },
        colors: [None, None, None, None],
    },
    Preset {
        name: "Rapid Profile",
        color: RED,
        keys: RAPID,
        lighting: Settings {
            brightness: 100,
            indicators: INDICATORS,
            effects: Vec::new(),
        },
        colors: [Some(&RED_KEYS), None, None, None],
    },
];
static MODE_FACTORY: Factory<ROWS, COLUMNS> = Factory {
    keymap: DefaultKeymap {
        layers: &LAYERS,
        variant_patches: &[],
    },
    presets: &PRESETS,
    fallback: Preset {
        name: "",
        color: BLACK,
        keys: TYPING,
        lighting: Settings {
            brightness: 255,
            indicators: INDICATORS,
            effects: Vec::new(),
        },
        colors: [None, None, None, None],
    },
    created: 2,
    global: GLOBAL,
    face: Face {
        mode: Mode::NoGamepad,
        keyboard: openwoot::usb::KeyboardFormat::NKeyRollover,
        speed: openwoot::usb::Speed::High,
        gamepad_polling: openwoot::usb::PollingRate::Every1Millisecond,
    },
};

type Lights = Lighting<(), ROWS, COLUMNS, { crate::lighting::PAGES }, { crate::lighting::LENGTH }>;

#[test]
fn the_mode_key_toggles_typing_and_rapid_and_applies_their_keys_and_lighting() {
    let world = World::new();
    let model = world.model_with(&MODE_FACTORY);
    let mut configurator = Configurator::new(&model, capabilities());
    let mut bench = Bench80::new(world.task(Mode::NoGamepad));
    let mut lighting = Lights::new(&MAP, &VARIANT, GEOMETRY, (), 0);
    let mut now_ms = 0;
    let mut show = |lighting: &mut Lights| {
        now_ms += 100;
        world.profile.apply(lighting, now_ms);
        lighting.set_output(true);
        lighting.update(now_ms, &());
        (lighting.brightness(), lighting.key_color(KEY_A))
    };
    assert_eq!(bench.keys.profiles().count(), 2);
    assert_eq!(
        bench.keys.tuning().actuation_point,
        travel_of(TYPING.actuation)
    );
    let (brightness, color) = show(&mut lighting);
    assert_eq!(brightness, 255);
    assert!(color.is_some_and(|color| color != RED && color != BLACK));
    for slot in [1, 0, 1] {
        bench.press(PROFILE_KEY, FULL);
        bench.cycle();
        block_on(select(configurator.serve(), ready(())));
        bench.press(PROFILE_KEY, 0);
        bench.cycle();
        let preset = &PRESETS[usize::from(slot)];
        assert_eq!(bench.keys.profiles().active(), slot);
        assert_eq!(
            block_on(configurator.profiles()).map(|summary| summary.active),
            Ok(Slot::new(slot))
        );
        assert_eq!(
            bench.keys.tuning().rapid_trigger_enabled,
            preset.keys.rapid_trigger_enabled
        );
        assert_eq!(
            bench.keys.tuning().actuation_point,
            travel_of(preset.keys.actuation)
        );
        let (brightness, color) = show(&mut lighting);
        assert_eq!(brightness, preset.lighting.brightness);
        assert_eq!(color == Some(RED), slot == 1, "profile {slot}: {color:?}");
    }
}
