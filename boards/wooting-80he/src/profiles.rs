use openwoot::{
    analog::SwitchModel,
    layout::LAYERS,
    lighting::{from_rgb565, lightbar_indicator},
    profile::{Edition, Factory, Preset},
};
use openwoot_protocol::{
    advanced::{self, Behaviour, DepthPriority, Resolution},
    global,
    heapless::Vec,
    keys::{self, Distance, Index, Layer, RapidTrigger, ScanMode, TapDuration},
    lightbar::Background,
    lighting::{
        AnalogLevel, Color, Effect, HueRange, Idle, Indicators, KeyColors, KeyTest, LevelMeter,
        RainbowWave, ReactiveColoring, ReactiveFade, Rgb, Settings,
    },
};

use crate::{FACE, KEY_COLUMNS, KEY_ROWS, keys::KEYMAP, lighting::LIGHTBAR_LEDS};

pub const CREATED: u8 = 2;
pub const EDITION_PRODUCT: &str = "Wooting 80HE TenZ";
pub const EDITION_CODES: [u8; 2] = [9, 99];

pub static FACTORY: Factory<KEY_ROWS, KEY_COLUMNS> = Factory {
    keymap: KEYMAP,
    presets: &PRESETS,
    fallback: OTHER,
    created: CREATED,
    global: GLOBAL,
    lightbar: lightbar_indicator(LIGHTBAR_COUNT, Background::Contrast, 5),
    face: FACE,
};

pub static EDITION: Edition<KEY_ROWS, KEY_COLUMNS> = Edition {
    codes: &EDITION_CODES,
    product: EDITION_PRODUCT,
    factory: Factory {
        keymap: KEYMAP,
        presets: &EDITION_PRESETS,
        fallback: OTHER,
        created: 3,
        global: GLOBAL,
        lightbar: lightbar_indicator(LIGHTBAR_COUNT, Background::ProfileColor, 1),
        face: FACE,
    },
    switch_model: match SwitchModel::new(1) {
        Some(switch_model) => switch_model,
        None => SwitchModel::DEFAULT,
    },
};

static PRESETS: [Preset; 4] = [
    TYPING,
    Preset {
        name: "Rapid Profile",
        color: RED,
        keys: RAPID_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting(
            255,
            [
                Effect::RainbowWave(RainbowWave {
                    hues: HueRange::From2To39,
                    period: 30,
                    forward: false,
                }),
                Effect::KeyTest(KeyTest { base_level: 255 }),
            ],
        ),
        colors: [Some(&RED_RUN), Some(&LAYER_ONE), None, None],
        lightbar_brightness: None,
    },
    Preset {
        name: "Analog Profile",
        color: NO_COLOR,
        keys: RAPID_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting(
            255,
            [Effect::LevelMeter(LevelMeter {
                base_level: 51,
                gamepad_curve: false,
            })],
        ),
        colors: [Some(&BLUE_RUN), None, None, None],
        lightbar_brightness: None,
    },
    ANALOG_GLOW,
];

static EDITION_PRESETS: [Preset; 4] = [
    Preset {
        name: "TenZ Profile",
        color: RED,
        keys: EDITION_KEYS,
        actuation_points: &EDITION_ACTUATION_POINTS,
        advanced: &[],
        lighting: lighting(EDITION_BRIGHTNESS, []),
        colors: [Some(&RED_RUN), Some(&LAYER_ONE), None, None],
        lightbar_brightness: Some(127),
    },
    TYPING,
    Preset {
        name: "TenZ Ultra Profile",
        color: RED,
        keys: keys::Settings {
            scan_mode: ScanMode::Parallel,
            ..EDITION_KEYS
        },
        actuation_points: &EDITION_ACTUATION_POINTS,
        advanced: &EDITION_KEY_PAIRS,
        lighting: lighting(EDITION_BRIGHTNESS, []),
        colors: [Some(&RED_RUN), Some(&LAYER_ONE), None, None],
        lightbar_brightness: Some(255),
    },
    ANALOG_GLOW,
];

const TYPING: Preset = Preset {
    name: "Typing Profile",
    color: BLUE,
    keys: TYPING_KEYS,
    actuation_points: &[],
    advanced: &[],
    lighting: lighting(
        255,
        [
            Effect::RainbowWave(RainbowWave {
                hues: HueRange::From113To204,
                period: 34,
                forward: false,
            }),
            reactive_fade(8, 0xFFFF),
            reactive_fade(4, 0xFFE0),
        ],
    ),
    colors: [Some(&BLUE_RUN), Some(&LAYER_ONE), None, None],
    lightbar_brightness: None,
};
const ANALOG_GLOW: Preset = Preset {
    name: "Analog Profile",
    color: NO_COLOR,
    keys: RAPID_KEYS,
    actuation_points: &[],
    advanced: &[],
    lighting: lighting(
        255,
        [Effect::AnalogGlow(AnalogLevel {
            base_level: 130,
            gamepad_curve: false,
        })],
    ),
    colors: [Some(&BLUE_RUN), None, None, None],
    lightbar_brightness: None,
};
const OTHER: Preset = Preset {
    name: "",
    color: NO_COLOR,
    keys: TYPING_KEYS,
    actuation_points: &[],
    advanced: &[],
    lighting: lighting(255, []),
    colors: [None; LAYERS],
    lightbar_brightness: None,
};
const GLOBAL: global::Settings = global::Settings {
    idle: Some(Idle {
        dim_after_minutes: 5,
        off_after_minutes: 10,
        off_enabled: true,
    }),
    num_lock_inverted: false,
    release_rebound_filter: false,
};
#[expect(
    clippy::cast_possible_truncation,
    reason = "the light bar has ten LEDs, a constant checked by the tests"
)]
const LIGHTBAR_COUNT: u8 = LIGHTBAR_LEDS.len() as u8;
const EDITION_BRIGHTNESS: u8 = 112;
const SHALLOW: Distance = Distance::from_fraction(26 * 257);
const DEEP: Distance = Distance::from_fraction(191 * 257);
const EDITION_ACTUATION_POINTS: [(Index, Distance); 9] = [
    (key(2, 2), SHALLOW),
    (key(3, 1), SHALLOW),
    (key(3, 2), SHALLOW),
    (key(3, 3), SHALLOW),
    (key(5, 0), SHALLOW),
    (key(0, 0), DEEP),
    (key(2, 0), DEEP),
    (key(3, 0), DEEP),
    (key(5, 6), DEEP),
];
const EDITION_KEY_PAIRS: [advanced::Key; 2] = [
    depth_priority(key(3, 1), key(3, 3)),
    depth_priority(key(2, 2), key(3, 2)),
];

const TYPING_KEYS: keys::Settings = keys::Settings {
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
const RAPID_KEYS: keys::Settings = keys::Settings {
    actuation: Distance::from_fraction(26 * 257),
    rapid_trigger_enabled: true,
    rapid_trigger: RapidTrigger {
        press: Distance::from_fraction(26 * 257),
        release: None,
        continuous: false,
    },
    ..TYPING_KEYS
};
const EDITION_KEYS: keys::Settings = keys::Settings {
    actuation: Distance::from_fraction(63 * 257),
    rapid_trigger_enabled: true,
    rapid_trigger: RapidTrigger {
        press: Distance::from_fraction(26 * 257),
        release: None,
        continuous: true,
    },
    ..TYPING_KEYS
};
const WHITE: Color = from_rgb565(0xFFFF).opaque();
const NO_COLOR: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};
const BLUE: Rgb = Rgb {
    red: 0x00,
    green: 0x00,
    blue: 0xFF,
};
const RED: Rgb = Rgb {
    red: 0xFF,
    green: 0x28,
    blue: 0x28,
};
const BASE_COLUMNS: usize = 21;
const BLUE_RUN_565: [[u16; BASE_COLUMNS]; KEY_ROWS] = [
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x07F1, 0x07F1, 0x07F1, 0x07F1, 0x6FE0, 0x6FE0, 0x6FE0, 0x6FE0,
    ],
];
const RED_RUN_565: [[u16; BASE_COLUMNS]; KEY_ROWS] = [
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
    [
        0xF800, 0xF800, 0xF800, 0xF800, 0xF800, 0xFA00, 0xFA00, 0xFA00, 0xFA00, 0xFBA0, 0xFBA0,
        0xFBA0, 0xFBA0, 0xFD60, 0xFD60, 0xFD60, 0xFD60, 0xFF60, 0xFF60, 0xFF60, 0xFF60,
    ],
];
const LAYER_ONE_565: [[u16; KEY_COLUMNS]; KEY_ROWS] = [
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0xFFFF, 0xFFFF, 0x07E0, 0x07E0, 0x07E0,
    ],
    [
        0x0000, 0xFFE0, 0xFFE0, 0xFFE0, 0xFFE0, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x001F, 0x001F,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x001F, 0x001F,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x001F, 0x001F, 0x001F, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x07E0, 0x07E0, 0x07E0,
        0x0000, 0x0000, 0x0000, 0x0000, 0xFD80, 0x0000,
    ],
    [
        0x0000, 0xF800, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0xFD80, 0xFD80, 0xFD80,
    ],
];

static BLUE_RUN: KeyColors = key_colors(&BLUE_RUN_565);
static RED_RUN: KeyColors = key_colors(&RED_RUN_565);
static LAYER_ONE: KeyColors = key_colors(&LAYER_ONE_565);

const fn lighting<const EFFECTS: usize>(brightness: u8, effects: [Effect; EFFECTS]) -> Settings {
    Settings {
        brightness,
        indicators: Indicators {
            caps_lock: WHITE,
            scroll_lock: WHITE,
            num_lock: WHITE,
            layer_lock: WHITE,
        },
        effects: Vec::from_array(effects),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "positions of the 6 x 17 key matrix fit a key index"
)]
const fn key(row: usize, column: usize) -> Index {
    Index::new((row * KEY_COLUMNS + column) as u8)
}

const fn depth_priority(first: Index, other: Index) -> advanced::Key {
    advanced::Key {
        key: first,
        layer: Layer::Base,
        behaviour: Behaviour::DepthPriority(DepthPriority {
            other,
            both_bottomed_out: Resolution::Both,
        }),
    }
}

const fn reactive_fade(step_ms: u8, rgb565: u16) -> Effect {
    Effect::ReactiveFade(ReactiveFade {
        base_level: 255,
        step_ms,
        coloring: ReactiveColoring::Fixed(from_rgb565(rgb565).opaque()),
        inverted: false,
    })
}

#[expect(
    clippy::indexing_slicing,
    reason = "runs only in constant evaluation, which rejects an index outside the fixed tables"
)]
const fn key_colors<const WIDTH: usize>(map: &[[u16; WIDTH]; KEY_ROWS]) -> KeyColors {
    let mut colors = [NO_COLOR; KEY_ROWS * KEY_COLUMNS];
    let mut row = 0;
    while row < KEY_ROWS {
        let mut column = 0;
        while column < KEY_COLUMNS {
            colors[row * KEY_COLUMNS + column] = from_rgb565(map[row][column]);
            column += 1;
        }
        row += 1;
    }
    Vec::from_array(colors)
}

#[cfg(test)]
mod tests {
    use openwoot::{
        keys::Key,
        lighting::{
            Bar, Canvas, EffectStack, Frame, KeyEvent, Lightbar, Painter, ProfileState, Signals,
        },
        profile::Unit,
        storage::{DeviceInfo, SerialNumber},
    };
    use openwoot_protocol::{
        keys::Layer,
        lightbar::{Effect, ProfileIndicator, Range},
    };

    use super::*;
    use crate::{
        NAME,
        keys::PROFILES,
        lighting::{ANSI, GEOMETRY, LEDS, PAGE_LENGTH, PAGES},
    };

    fn unit_with_edition(edition: Option<u8>) -> Unit<KEY_ROWS, KEY_COLUMNS> {
        let info = DeviceInfo {
            serial: SerialNumber {
                edition,
                ..SerialNumber::default()
            },
            aux: None,
            physical_layout: None,
            lock_mode: None,
        };
        EDITION.unit(Unit::regular(NAME, &FACTORY), Some(&info))
    }

    #[test]
    fn a_tenz_serial_selects_the_edition_product_profiles_and_switch_model() {
        for code in EDITION_CODES {
            let unit = unit_with_edition(Some(code));
            assert_eq!(unit.product, "Wooting 80HE TenZ");
            assert_eq!(unit.edition, Some(code));
            assert_eq!(unit.switch_model.id(), 1);
            assert_eq!(unit.factory.created, 3);
            assert_eq!(unit.factory.preset(0).info().name.as_str(), "TenZ Profile");
            assert_eq!(
                unit.factory.preset(1).info().name.as_str(),
                "Typing Profile"
            );
            assert_eq!(
                unit.factory.preset(2).info().name.as_str(),
                "TenZ Ultra Profile"
            );
            assert_eq!(unit.factory.preset(2).keys.scan_mode, ScanMode::Parallel);
            assert_eq!(unit.factory.preset(2).advanced_keys().len(), 2);
            let overrides = unit.factory.preset(0).overrides(KEY_ROWS * KEY_COLUMNS);
            assert_eq!(overrides[3 * KEY_COLUMNS + 1].actuation, Some(SHALLOW));
            assert_eq!(overrides[5 * KEY_COLUMNS + 6].actuation, Some(DEEP));
            assert_eq!(overrides[1].actuation, None);
            assert_eq!(unit.factory.preset(0).lighting.brightness, 112);
            assert_eq!(unit.factory.preset(0).lightbar().brightness, Some(127));
            assert_eq!(unit.factory.preset(1).lightbar().brightness, None);
            assert_eq!(unit.factory.preset(2).lightbar().brightness, Some(255));
        }
        for edition in [None, Some(1), Some(10)] {
            let unit = unit_with_edition(edition);
            assert_eq!(unit.product, "Wooting 80HE");
            assert_eq!(unit.edition, None);
            assert_eq!(unit.switch_model.id(), 0);
            assert_eq!(unit.factory.created, 2);
        }
        let unread = EDITION.unit(Unit::regular(NAME, &FACTORY), None);
        assert_eq!(unread.product, NAME);
    }

    #[test]
    fn the_factory_light_bar_shows_the_profile_over_the_whole_bar() {
        let indicator = |entries: &openwoot_protocol::lightbar::Entries| match entries.first() {
            Some(entry) => match &entry.effect {
                Effect::ProfileIndicator(indicator) => Some(*indicator),
                _ => None,
            },
            None => None,
        };
        assert_eq!(
            indicator(&FACTORY.lightbar),
            Some(ProfileIndicator {
                range: Range { first: 0, last: 9 },
                background: Background::Contrast,
                segments: 5,
            })
        );
        assert_eq!(
            indicator(&EDITION.factory.lightbar),
            Some(ProfileIndicator {
                range: Range { first: 0, last: 9 },
                background: Background::ProfileColor,
                segments: 1,
            })
        );
        let preset = FACTORY.preset(0);
        let settings = preset.lightbar();
        assert!(settings.enabled && settings.use_global && settings.entries.is_empty());
        assert_eq!(settings.brightness, None);

        let mut bar = Bar::new(&LIGHTBAR_LEDS, &LEDS, KEY_COLUMNS);
        bar.configure(&settings, &FACTORY.lightbar, 200);
        bar.profile_changed(ProfileState {
            slot: 0,
            count: 2,
            color: preset.color,
        });
        let mut frame = Frame::<PAGES, PAGE_LENGTH>::new();
        let mut canvas = Painter::new(&mut frame, &LEDS, &ANSI, None);
        assert!(bar.needs_render(16));
        bar.render(&mut canvas, 16);
        let shown: [Rgb; 10] = LIGHTBAR_LEDS.map(|led| canvas.led(led).unwrap_or(NO_COLOR));
        let green = Rgb {
            red: 0,
            green: 0xFF,
            blue: 0,
        };
        assert_eq!(shown[..2], [BLUE; 2]);
        assert_eq!(shown[2..], [green; 8]);
        assert_eq!(bar.brightness(), 200);
    }

    const STEP_MS: u32 = 16;
    const KEY_EVENTS: [(u32, Key, KeyEvent); 4] = [
        (3, Key::new(3, 1), KeyEvent::Pressed),
        (10, Key::new(3, 1), KeyEvent::Released),
        (12, Key::new(2, 5), KeyEvent::Pressed),
        (30, Key::new(2, 5), KeyEvent::Released),
    ];
    const TYPING_FRAMES: [u64; 60] = [
        0xdc8e_7cd2_4515_dcba,
        0xdc8e_7cd2_4515_dcba,
        0x69f9_34f5_43e9_3ec7,
        0x69f9_34f5_43e9_3ec7,
        0x02a4_0558_5e6a_8a77,
        0x02a4_0558_5e6a_8a77,
        0x10b8_3dd2_323c_47ab,
        0x10b8_3dd2_323c_47ab,
        0xfc84_edad_3e3c_f619,
        0xfc84_edad_3e3c_f619,
        0xabfc_958b_f830_0e7b,
        0xc668_32fd_87e2_c944,
        0xaa17_8328_623d_286e,
        0xaa17_8328_623d_286e,
        0xbc55_f636_68d0_46a5,
        0xbc55_f636_68d0_46a5,
        0x6ffb_ef1c_2fee_b60b,
        0x6ffb_ef1c_2fee_b60b,
        0xf985_0a6e_d1a2_6c4b,
        0xf985_0a6e_d1a2_6c4b,
        0xf30a_51ab_2805_64f3,
        0xf30a_51ab_2805_64f3,
        0xe04e_8f8a_0a3c_c0d1,
        0xe04e_8f8a_0a3c_c0d1,
        0x58eb_c1ba_2df4_dabb,
        0x58eb_c1ba_2df4_dabb,
        0xc48e_a7e6_0018_258b,
        0xc48e_a7e6_0018_258b,
        0xc48e_a7e6_0018_258b,
        0xef51_6c42_35df_5887,
        0xef51_6c42_35df_5887,
        0xa40d_7b4b_72c7_96a5,
        0xa40d_7b4b_72c7_96a5,
        0x55af_47ad_6562_7f11,
        0x55af_47ad_6562_7f11,
        0x486e_27b4_aaef_d56f,
        0x486e_27b4_aaef_d56f,
        0x8bdd_d65a_2eed_1fcb,
        0x8bdd_d65a_2eed_1fcb,
        0x6bfc_a682_09e2_15d9,
        0x635b_a7f7_d629_d054,
        0xcf92_c294_df7c_19c6,
        0xdab6_b126_b814_2614,
        0x142f_7988_8540_d7cb,
        0x8bf8_5ee5_b38b_b141,
        0x93da_ac66_5b5e_0d2d,
        0x94d3_9f53_e088_32a7,
        0xf513_a53d_f572_a33d,
        0xc727_6a01_77d7_ef66,
        0xdfea_6039_faa0_5125,
        0x8b0d_41ad_fbc0_ba99,
        0x9a74_2542_dbf8_01a5,
        0x458f_3481_acfe_bbbc,
        0xbc02_27cc_735e_c4ff,
        0x9cfc_b08f_36bb_2ee3,
        0xf4e5_2032_3b57_b236,
        0x4b4c_61d6_91bf_5006,
        0x453c_f0bb_e422_6d39,
        0x9a63_9768_0812_5dcf,
        0xea97_7d27_2005_6337,
    ];
    const RAPID_FRAMES: [u64; 60] = [
        0x227a_dc7b_76f2_761f,
        0x4aa3_b43e_c67a_fd17,
        0x4ae0_9b51_678e_3e8d,
        0xe799_cde0_b49a_3f47,
        0xe799_cde0_b49a_3f47,
        0xcf7b_10db_5947_69a9,
        0xcf7b_10db_5947_69a9,
        0x20b2_3d53_30ce_1699,
        0x20b2_3d53_30ce_1699,
        0x219d_0662_cc7e_384d,
        0x674e_9081_11cf_8ccd,
        0xff38_9076_13eb_eb23,
        0x105f_13d2_1776_cea5,
        0x105f_13d2_1776_cea5,
        0x69ee_20a5_2f79_f8ff,
        0x69ee_20a5_2f79_f8ff,
        0x926a_8c71_b2f9_07e3,
        0x926a_8c71_b2f9_07e3,
        0xa055_a2c3_0a66_cf0f,
        0xa055_a2c3_0a66_cf0f,
        0xa4f9_46f1_da98_8e51,
        0x4544_40e6_9972_2867,
        0x4544_40e6_9972_2867,
        0xd7c1_bd8d_1c31_4ed3,
        0xd7c1_bd8d_1c31_4ed3,
        0x1427_fb10_94f0_e095,
        0x1427_fb10_94f0_e095,
        0xd479_92c8_b6ab_7147,
        0xd479_92c8_b6ab_7147,
        0xbf05_4f45_61ab_f8e5,
        0xbf05_4f45_61ab_f8e5,
        0x4d1d_cd96_cf15_74fb,
        0x4d1d_cd96_cf15_74fb,
        0x4d1d_cd96_cf15_74fb,
        0x8b91_142f_0f99_d891,
        0x8b91_142f_0f99_d891,
        0x34d7_0862_fa1a_96c1,
        0x34d7_0862_fa1a_96c1,
        0xebbd_2138_7ada_811b,
        0xebbd_2138_7ada_811b,
        0x796b_abda_456b_58ed,
        0x796b_abda_456b_58ed,
        0x16f6_7333_0f11_7bd9,
        0x4bbd_bad9_3215_5f77,
        0x4bbd_bad9_3215_5f77,
        0x3c00_73b6_27dd_b2d3,
        0x3c00_73b6_27dd_b2d3,
        0x3862_bc25_fbc6_53e5,
        0x3862_bc25_fbc6_53e5,
        0xba60_5a58_1d8e_8b89,
        0xba60_5a58_1d8e_8b89,
        0x04a9_a7cf_cae1_02bb,
        0x04a9_a7cf_cae1_02bb,
        0xcdfd_fe14_4173_998d,
        0xe725_e99a_c1a8_0cb1,
        0xe725_e99a_c1a8_0cb1,
        0x3079_9a44_1ec4_9737,
        0x3079_9a44_1ec4_9737,
        0x3674_cedb_fa02_59a5,
        0x3674_cedb_fa02_59a5,
    ];

    fn frames_of(preset: &Preset) -> [u64; 60] {
        let mut layers = EffectStack::<KEY_ROWS, KEY_COLUMNS>::new();
        layers.apply(&preset.lighting.effects);
        let mut digests = [0; 60];
        for (number, digest) in (1..).zip(digests.iter_mut()) {
            let now_ms = 1000 + STEP_MS * number;
            for (_, key, event) in KEY_EVENTS.iter().filter(|(at, ..)| *at == number) {
                layers.event(*key, *event, now_ms, &ANSI);
            }
            let mut frame = Frame::<PAGES, PAGE_LENGTH>::new();
            let mut canvas = Painter::new(&mut frame, &LEDS, &ANSI, None);
            let base = preset.colors.first().copied().flatten();
            for row in 0..KEY_ROWS {
                for column in 0..KEY_COLUMNS {
                    let color = base
                        .and_then(|colors| colors.get(row * KEY_COLUMNS + column))
                        .copied()
                        .unwrap_or(NO_COLOR);
                    canvas.set_key(row, column, color);
                }
            }
            layers.render(&mut canvas, GEOMETRY, STEP_MS);
            *digest = (0..KEY_ROWS)
                .flat_map(|row| (0..KEY_COLUMNS).map(move |column| (row, column)))
                .map(|(row, column)| canvas.key(row, column).unwrap_or_default())
                .flat_map(|color| [color.red, color.green, color.blue])
                .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
                    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
                });
        }
        digests
    }

    #[test]
    fn the_factory_profiles_draw_the_frames_of_the_reference_firmware() {
        assert_eq!(frames_of(FACTORY.preset(0)), TYPING_FRAMES);
        assert_eq!(frames_of(FACTORY.preset(1)), RAPID_FRAMES);
    }

    #[test]
    fn a_fresh_keyboard_has_a_typing_and_a_rapid_profile() {
        assert_eq!(FACTORY.created, 2);
        assert_eq!(FACTORY.preset(0).info().name.as_str(), "Typing Profile");
        assert_eq!(FACTORY.preset(1).info().name.as_str(), "Rapid Profile");
        assert!(!FACTORY.preset(0).keys.rapid_trigger_enabled);
        assert!(FACTORY.preset(1).keys.rapid_trigger_enabled);
        assert_eq!(FACTORY.preset(0).keys.scan_mode, ScanMode::Single);
        assert_eq!(FACTORY.preset(9).lighting.effects.len(), 0);
    }

    #[test]
    fn the_colour_maps_follow_the_key_matrix() {
        let appearance = FACTORY.preset(0).appearance();
        let base = appearance.colors.layer(Layer::Base).unwrap_or(&BLUE_RUN);
        assert_eq!(base.len(), KEY_ROWS * KEY_COLUMNS);
        assert_eq!(base.first(), Some(&BLUE));
        assert_eq!(
            appearance
                .colors
                .layer(Layer::One)
                .and_then(|colors| colors.get(KEY_COLUMNS + 1)),
            Some(&from_rgb565(0xFFE0))
        );
    }

    #[test]
    fn the_light_bar_count_matches_the_led_table() {
        assert_eq!(usize::from(LIGHTBAR_COUNT), LIGHTBAR_LEDS.len());
    }

    fn lit_keys(preset: &Preset) -> usize {
        let mut layers = EffectStack::<KEY_ROWS, KEY_COLUMNS>::new();
        layers.apply(&preset.lighting.effects);
        let mut frame = Frame::<PAGES, PAGE_LENGTH>::new();
        let mut canvas = Painter::new(&mut frame, &LEDS, &ANSI, None);
        let base = preset.colors.first().copied().flatten();
        for row in 0..KEY_ROWS {
            for column in 0..KEY_COLUMNS {
                let color = base
                    .and_then(|colors| colors.get(row * KEY_COLUMNS + column))
                    .copied()
                    .unwrap_or(NO_COLOR);
                canvas.set_key(row, column, color);
            }
        }
        layers.render(&mut canvas, GEOMETRY, STEP_MS);
        (0..KEY_ROWS)
            .flat_map(|row| (0..KEY_COLUMNS).map(move |column| (row, column)))
            .filter_map(|(row, column)| canvas.key(row, column))
            .filter(|color| *color != NO_COLOR)
            .count()
    }

    #[test]
    fn every_profile_slot_lights_every_key_led() {
        let key_leds = ANSI.leds.iter().flatten().flatten().count();
        for slot in 0..PROFILES {
            assert_eq!(lit_keys(FACTORY.preset(slot)), key_leds, "slot {slot}");
            assert_eq!(
                lit_keys(EDITION.factory.preset(slot)),
                key_leds,
                "edition slot {slot}"
            );
        }
    }

    #[test]
    fn each_factory_profile_has_its_base_colours() {
        let regular = |slot| FACTORY.preset(slot).colors[0];
        assert_eq!(
            [regular(0), regular(1), regular(2), regular(3)],
            [
                Some(&BLUE_RUN),
                Some(&RED_RUN),
                Some(&BLUE_RUN),
                Some(&BLUE_RUN)
            ]
        );
        let edition = |slot| EDITION.factory.preset(slot).colors[0];
        assert_eq!(
            [edition(0), edition(1), edition(2), edition(3)],
            [
                Some(&RED_RUN),
                Some(&BLUE_RUN),
                Some(&RED_RUN),
                Some(&BLUE_RUN)
            ]
        );
    }
}
