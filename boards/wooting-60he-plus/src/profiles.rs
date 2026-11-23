use openwoot::{
    layout::LAYERS,
    lighting::from_rgb565,
    profile::{Factory, Preset},
};
use openwoot_protocol::{
    global,
    heapless::Vec,
    keys::{self, Distance, RapidTrigger, ScanMode, TapDuration},
    lighting::{
        Color, Effect, HueRange, Idle, Indicators, KeyColors, KeyTest, RainbowWave,
        ReactiveColoring, ReactiveFade, Rgb, Settings,
    },
};

use crate::{FACE, KEY_COLUMNS, KEY_ROWS, keys::KEYMAP};

pub const CREATED: u8 = 2;

pub static FACTORY: Factory<KEY_ROWS, KEY_COLUMNS> = Factory {
    keymap: KEYMAP,
    presets: &PRESETS,
    fallback: Preset {
        name: "",
        color: NO_COLOR,
        keys: OTHER_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting([]),
        colors: [None; LAYERS],
        lightbar_brightness: None,
    },
    created: CREATED,
    global: global::Settings {
        idle: Some(Idle {
            dim_after_minutes: 5,
            off_after_minutes: 10,
            off_enabled: true,
        }),
        num_lock_inverted: false,
        release_rebound_filter: false,
    },
    lightbar: Vec::new(),
    face: FACE,
};

static PRESETS: [Preset; 4] = [
    Preset {
        name: "Typing Profile",
        color: BLUE,
        keys: TYPING_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting([
            Effect::RainbowWave(RainbowWave {
                hues: HueRange::FullCircle,
                period: 65,
                forward: false,
            }),
            reactive_fade(6, 0xFFFF),
        ]),
        colors: [Some(&COLOR_RUN), Some(&LAYER_ONE), Some(&LAYER_TWO), None],
        lightbar_brightness: None,
    },
    Preset {
        name: "Rapid Profile",
        color: RED,
        keys: RAPID_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting([
            Effect::RainbowWave(RainbowWave {
                hues: HueRange::From2To39,
                period: 30,
                forward: false,
            }),
            Effect::KeyTest(KeyTest { base_level: 255 }),
        ]),
        colors: [Some(&BLUE_RUN), Some(&LAYER_ONE), Some(&LAYER_TWO), None],
        lightbar_brightness: None,
    },
    uncreated(),
    uncreated(),
];

const TYPING_KEYS: keys::Settings = keys::Settings {
    actuation: Distance::from_fraction(102 * 257),
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
        press: Distance::from_fraction(13 * 257),
        release: Some(Distance::from_fraction(35 * 257)),
        continuous: false,
    },
    ..TYPING_KEYS
};
const OTHER_KEYS: keys::Settings = keys::Settings {
    actuation: Distance::from_fraction(63 * 257),
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
const COLOR_RUN_565: [[u16; BASE_COLUMNS]; KEY_ROWS] = [
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0xF817, 0x781F, 0x001F, 0x03FF, 0x07FF, 0x07EF, 0x07E0, 0x7FE0, 0xDFE0, 0xFE00, 0xFE00,
        0xFBE0, 0xF9C0, 0xF800, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0xF817, 0x781F, 0x001F, 0x03FF, 0x07FF, 0x07EF, 0x07E0, 0x7FE0, 0xDFE0, 0xFE00, 0xFE00,
        0xFBE0, 0xF9C0, 0xF800, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0xF817, 0x781F, 0x001F, 0x03FF, 0x07FF, 0x07EF, 0x07E0, 0x7FE0, 0xDFE0, 0xFE00, 0xFE00,
        0xFBE0, 0xF9C0, 0xF800, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0xF817, 0xF817, 0x781F, 0x001F, 0x03FF, 0x07FF, 0x07EF, 0x07E0, 0x7FE0, 0xDFE0, 0xFE00,
        0xFE00, 0xFBE0, 0xF9C0, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0xF817, 0xF817, 0x781F, 0x0000, 0x001F, 0x07FF, 0x07EF, 0x07E0, 0x7FE0, 0xDFE0, 0xFE00,
        0xFE00, 0xFBE0, 0xF9C0, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
];
const BLUE_RUN_565: [[u16; BASE_COLUMNS]; KEY_ROWS] = [
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x071F, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x071F, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F, 0x071F,
        0x071F, 0x071F, 0x071F, 0x6FE0, 0x6FE0, 0x6FE0, 0x0000, 0x0000, 0x0000, 0x6FE0,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F,
        0x071F, 0x07F1, 0x071F, 0x0FE0, 0x0000, 0x6FE0, 0x0000, 0x0000, 0x0000, 0x0000,
    ],
    [
        0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x001F, 0x03BF, 0x03BF, 0x03BF, 0x03BF, 0x071F,
        0x071F, 0x071F, 0x071F, 0x0000, 0x0000, 0x0000, 0x6FE0, 0x0000, 0x0000, 0x6FE0,
    ],
];
const LAYER_ONE_565: [[u16; KEY_COLUMNS]; KEY_ROWS] = [
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0xF815, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F, 0x781F,
        0x781F, 0x781F, 0xF815,
    ],
    [
        0xF815, 0x0000, 0xFD80, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0xFFE0,
        0xFFE0, 0xFFE0, 0x0000,
    ],
    [
        0x52AA, 0xFD80, 0xFD80, 0xFD80, 0x0000, 0x0000, 0x0000, 0x001F, 0x001F, 0x001F, 0xFFFF,
        0xFFFF, 0x0000, 0xFFE0,
    ],
    [
        0x52AA, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x07E0, 0x07E0, 0x07E0,
        0x07FC, 0x0000, 0x52AA,
    ],
    [
        0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x52AA, 0x07FC,
        0x07FC, 0x07FC, 0x52AA,
    ],
];
const LAYER_TWO_565: [[u16; KEY_COLUMNS]; KEY_ROWS] = [
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
    [
        0x0000, 0xF800, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000,
        0x0000, 0x0000, 0x0000,
    ],
];

static COLOR_RUN: KeyColors = key_colors(&COLOR_RUN_565);
static BLUE_RUN: KeyColors = key_colors(&BLUE_RUN_565);
static LAYER_ONE: KeyColors = key_colors(&LAYER_ONE_565);
static LAYER_TWO: KeyColors = key_colors(&LAYER_TWO_565);

const fn uncreated() -> Preset {
    Preset {
        name: "",
        color: NO_COLOR,
        keys: RAPID_KEYS,
        actuation_points: &[],
        advanced: &[],
        lighting: lighting([]),
        colors: [Some(&COLOR_RUN), None, None, None],
        lightbar_brightness: None,
    }
}

const fn lighting<const EFFECTS: usize>(effects: [Effect; EFFECTS]) -> Settings {
    Settings {
        brightness: 255,
        indicators: Indicators {
            caps_lock: WHITE,
            scroll_lock: WHITE,
            num_lock: WHITE,
            layer_lock: WHITE,
        },
        effects: Vec::from_array(effects),
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
        lighting::{Canvas, EffectStack, Frame, KeyEvent, Painter},
    };
    use openwoot_protocol::keys::Layer;

    use super::*;
    use crate::{
        keys::PROFILES,
        lighting::{ANSI, GEOMETRY, LEDS, PAGE_LENGTH, PAGES},
    };

    const STEP_MS: u32 = 16;
    const KEY_EVENTS: [(u32, Key, KeyEvent); 4] = [
        (3, Key::new(3, 1), KeyEvent::Pressed),
        (10, Key::new(3, 1), KeyEvent::Released),
        (12, Key::new(2, 5), KeyEvent::Pressed),
        (30, Key::new(2, 5), KeyEvent::Released),
    ];
    const TYPING_FRAMES: [u64; 60] = [
        0x31b7_44f3_63bc_29cb,
        0x31b7_44f3_63bc_29cb,
        0x7c5c_3926_893a_be6e,
        0xf0fd_f4bf_10b3_ef97,
        0xf0fd_f4bf_10b3_ef97,
        0xf0fd_f4bf_10b3_ef97,
        0xf0fd_f4bf_10b3_ef97,
        0xbcfd_0e4c_6995_c0ca,
        0xbcfd_0e4c_6995_c0ca,
        0xbcfd_0e4c_6995_c0ca,
        0xbcfd_0e4c_6995_c0ca,
        0x9095_450a_e638_442b,
        0x9095_450a_e638_442b,
        0x9095_450a_e638_442b,
        0x9095_450a_e638_442b,
        0x0eef_d7d0_0ddf_f0ba,
        0x0eef_d7d0_0ddf_f0ba,
        0x0eef_d7d0_0ddf_f0ba,
        0x0eef_d7d0_0ddf_f0ba,
        0x3caa_318f_5351_b3ce,
        0x3caa_318f_5351_b3ce,
        0x3caa_318f_5351_b3ce,
        0x3caa_318f_5351_b3ce,
        0x83c1_fa6a_f506_f876,
        0x83c1_fa6a_f506_f876,
        0x83c1_fa6a_f506_f876,
        0x83c1_fa6a_f506_f876,
        0x231e_1597_51eb_4807,
        0x231e_1597_51eb_4807,
        0x231e_1597_51eb_4807,
        0x231e_1597_51eb_4807,
        0x1e7f_7e1e_863a_0de6,
        0x1e7f_7e1e_863a_0de6,
        0x1e7f_7e1e_863a_0de6,
        0x1e7f_7e1e_863a_0de6,
        0x9f88_9aff_b24c_1ed5,
        0x9f88_9aff_b24c_1ed5,
        0x9f88_9aff_b24c_1ed5,
        0x9f88_9aff_b24c_1ed5,
        0x20d1_e063_87f5_1007,
        0x20d1_e063_87f5_1007,
        0x20d1_e063_87f5_1007,
        0x20d1_e063_87f5_1007,
        0xcb4e_9007_4193_ecb8,
        0xcb4e_9007_4193_ecb8,
        0xcb4e_9007_4193_ecb8,
        0xcb4e_9007_4193_ecb8,
        0xf6f3_e8d0_ca12_8ed6,
        0xf6f3_e8d0_ca12_8ed6,
        0xf6f3_e8d0_ca12_8ed6,
        0xf6f3_e8d0_ca12_8ed6,
        0x6115_bd23_0544_bc9e,
        0x6115_bd23_0544_bc9e,
        0x6115_bd23_0544_bc9e,
        0x6115_bd23_0544_bc9e,
        0x4557_32fa_a4c5_385e,
        0x4557_32fa_a4c5_385e,
        0x4557_32fa_a4c5_385e,
        0x4557_32fa_a4c5_385e,
        0x51d3_7037_b5b9_88bb,
    ];
    const RAPID_FRAMES: [u64; 60] = [
        0xea07_e981_fd06_fa3f,
        0xd223_5da7_7187_dcff,
        0x5e32_6426_cf5d_d56a,
        0x0afc_1b9d_711f_56f6,
        0x0afc_1b9d_711f_56f6,
        0x9e2d_c8f4_747e_5f6a,
        0x9e2d_c8f4_747e_5f6a,
        0x9b4b_e0e6_2dd5_e3a6,
        0x9b4b_e0e6_2dd5_e3a6,
        0x4bfa_6bf5_7e21_0f27,
        0x22bd_554b_4626_42b5,
        0x95c3_d230_2c71_3852,
        0x01f0_f730_3d9d_1ef0,
        0x01f0_f730_3d9d_1ef0,
        0xd1c4_b71a_e655_2314,
        0xd1c4_b71a_e655_2314,
        0x6396_1691_d961_a92c,
        0x6396_1691_d961_a92c,
        0x260e_934d_7aef_27a4,
        0x260e_934d_7aef_27a4,
        0x2583_292d_0dad_f2e4,
        0xcefb_513e_9925_7694,
        0xcefb_513e_9925_7694,
        0xee9a_52b6_6da7_dd8a,
        0xee9a_52b6_6da7_dd8a,
        0xfdf6_164d_60eb_258c,
        0xfdf6_164d_60eb_258c,
        0x2e31_e69d_b147_790a,
        0x2e31_e69d_b147_790a,
        0xbee5_51a0_f80b_393d,
        0xbee5_51a0_f80b_393d,
        0x7493_98f6_35f5_eeeb,
        0x31a4_0e3b_561b_0d53,
        0x31a4_0e3b_561b_0d53,
        0xbe1e_620f_2d03_8bbf,
        0xbe1e_620f_2d03_8bbf,
        0x7844_b74a_fe77_f40d,
        0x7844_b74a_fe77_f40d,
        0x3880_72ff_2d45_0173,
        0x3880_72ff_2d45_0173,
        0x2141_adad_9190_5f99,
        0x2141_adad_9190_5f99,
        0xc604_9b5b_d483_3801,
        0x2f2e_9533_3b5a_199f,
        0x2f2e_9533_3b5a_199f,
        0x66c1_9b44_a55d_9655,
        0x66c1_9b44_a55d_9655,
        0xf6dc_e40f_9391_4b85,
        0xf6dc_e40f_9391_4b85,
        0x7f84_1e40_3cb8_5251,
        0x7f84_1e40_3cb8_5251,
        0xd12f_f702_4d68_59b7,
        0xd12f_f702_4d68_59b7,
        0xfb49_a61c_622a_960d,
        0xb5ba_ef19_2fa3_dc45,
        0xb5ba_ef19_2fa3_dc45,
        0xed2d_d80c_f7cc_aba3,
        0xed2d_d80c_f7cc_aba3,
        0x3ad7_faef_b5ec_cc69,
        0x3ad7_faef_b5ec_cc69,
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
        let base = appearance.colors.layer(Layer::Base).unwrap_or(&COLOR_RUN);
        assert_eq!(base.len(), KEY_ROWS * KEY_COLUMNS);
        assert_eq!(base.get(KEY_COLUMNS), Some(&from_rgb565(0xF817)));
        assert_eq!(
            appearance
                .colors
                .layer(Layer::Two)
                .and_then(|colors| colors.get(5 * KEY_COLUMNS + 1)),
            Some(&from_rgb565(0xF800))
        );
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
        }
    }

    #[test]
    fn each_factory_profile_has_its_base_colours() {
        let base = |slot| FACTORY.preset(slot).colors[0];
        assert_eq!(
            [base(0), base(1), base(2), base(3)],
            [
                Some(&COLOR_RUN),
                Some(&BLUE_RUN),
                Some(&COLOR_RUN),
                Some(&COLOR_RUN)
            ]
        );
    }
}
