use openwoot_protocol::{
    ENDPOINT_LIST, Endpoint, GetAdvancedKeys, GetCapabilities, GetKeyColors, GetKeymap,
    GetLightbar, GetSwitches, MAX_REPORT_LEN, SetAdvancedKeys, SetDeviceSettings,
    SetGamepadMapping, SetGamepadSettings, SetGlobalLightbar, SetGlobalSettings, SetHostSignals,
    SetKeyColors, SetKeyOverrides, SetKeySettings, SetKeymap, SetLayout, SetLightbar, SetLighting,
    SetProfileInfo, SetSwitches,
    advanced::{
        self, Behaviour, DepthPriority, DynamicKeystroke, Interrupt, ModTap, Resolution, Socd,
        Stroke, StrokeEvent,
    },
    config::{Capabilities, GlobalSettings, Invalid, MAX_KEYS, Rejection, Validate},
    decode_message, decode_response,
    device::{self, Layout},
    encode_message, encode_request,
    gamepad::{self, Control, CurvePoint, Normalization},
    heapless::{String, Vec},
    keys::{
        self, Action, Distance, Function, Index, Layer, MouseButton, Override, Position,
        RapidTrigger, RapidTriggerOverride, ScanMode, SystemControl, TapDuration,
    },
    lightbar::{
        self, Background, Blink, Builtin, Comparator, Comparison, Condition, Custom, Entry, Group,
        HostSignal, KeyRect, Logic, Meter, Operand, ProfileIndicator, Range, Signal, Solid, Target,
        Term, Waveform,
    },
    lighting::{
        self, AnalogLevel, Color, Effect, HueCycle, HueRange, Idle, Indicators, ReactiveColoring,
        ReactiveFade, Rgb, Scanner, ScannerDirection,
    },
    profile::{self, Change, LayerChange, LayerRef, Slot},
    switches::{self, Switch},
};

const SEQUENCE: u16 = 0x0102;
const DONE: Result<(), Rejection> = Ok(());

macro_rules! round_trip {
    ($endpoint:ty, $request:expr, $response:expr) => {{
        let request = $request;
        let response = $response;
        let mut report = [0; MAX_REPORT_LEN];
        let encoded = encode_request::<$endpoint>(SEQUENCE, &request, &mut report).unwrap();
        let message = decode_message(encoded).unwrap().unwrap();
        assert_eq!(
            postcard::from_bytes::<<$endpoint as Endpoint>::Request>(message.body).unwrap(),
            request
        );
        let mut reply = [0; MAX_REPORT_LEN];
        let len = encode_message(
            &message.reply_header(<$endpoint as Endpoint>::RESP_KEY),
            &response,
            &mut reply,
        )
        .unwrap()
        .len();
        assert_eq!(
            decode_response::<$endpoint>(SEQUENCE, &reply[..len]),
            Ok(Some(response))
        );
    }};
}

fn capabilities() -> Capabilities {
    Capabilities {
        key_rows: 6,
        key_columns: 17,
        profiles: 4,
        layouts: [Layout::Ansi, Layout::Iso, Layout::Jis]
            .into_iter()
            .collect(),
        lightbar_leds: 10,
        host_signals: true,
        switch_models: 49,
        edition: Some(1),
    }
}

fn key_count() -> usize {
    capabilities().key_count()
}

fn filled<T: Clone, const N: usize>(value: T, len: usize) -> Vec<T, N> {
    core::iter::repeat_n(value, len).collect()
}

fn widest_action() -> Action {
    Action::Consumer(0x02A2)
}

fn widest_override() -> Override {
    Override {
        actuation: Some(Distance::FULL_TRAVEL),
        rapid_trigger: RapidTriggerOverride::With(RapidTrigger {
            press: Distance::FULL_TRAVEL,
            release: Some(Distance::FULL_TRAVEL),
            continuous: true,
        }),
    }
}

fn widest_advanced_key() -> advanced::Key {
    let stroke = Stroke {
        action: widest_action(),
        on_press: StrokeEvent::Tap,
        on_bottom_out: StrokeEvent::Tap,
        on_lift: StrokeEvent::Tap,
        on_release: StrokeEvent::Tap,
    };
    advanced::Key {
        key: Index::new(u8::MAX),
        layer: Layer::Three,
        behaviour: Behaviour::DynamicKeystroke(DynamicKeystroke {
            strokes: [stroke; advanced::STROKES],
            bottom_out: Some(Distance::FULL_TRAVEL),
        }),
    }
}

fn widest_lightbar_entry() -> Entry {
    let signal = Signal::Host(u32::MAX);
    let term = Term::Compare(Comparison {
        signal,
        comparator: Comparator::LessOrEqual,
        operand: Operand::Signal(signal),
        hold_ms: Some(u32::MAX),
    });
    let group = Group {
        logic: Logic::Any,
        terms: filled(term, lightbar::MAX_TERMS),
    };
    Entry {
        condition: Some(Condition {
            logic: Logic::Any,
            groups: filled(group, lightbar::MAX_GROUPS),
        }),
        effect: lightbar::Effect::Custom(Custom {
            colors: filled(white(), lightbar::MAX_LEDS),
            range: Range {
                first: u8::MAX,
                last: u8::MAX,
            },
        }),
    }
}

fn white() -> Rgb {
    Rgb {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    }
}

fn opaque(red: u8, green: u8, blue: u8) -> Color {
    Rgb { red, green, blue }.opaque()
}

fn lighting_settings() -> lighting::Settings {
    lighting::Settings {
        brightness: 200,
        indicators: Indicators {
            caps_lock: opaque(255, 255, 255),
            scroll_lock: opaque(0, 255, 0),
            num_lock: opaque(0, 0, 255),
            layer_lock: Color::TRANSPARENT,
        },
        effects: [
            Effect::Scanner(Scanner {
                width: 3,
                steps_per_second: 15,
                outside_level: 40,
                direction: ScannerDirection::Right,
            }),
            Effect::ReactiveFade(ReactiveFade {
                base_level: 10,
                step_ms: 4,
                coloring: ReactiveColoring::Fixed(opaque(255, 40, 40)),
                inverted: false,
            }),
            Effect::AnalogGlow(AnalogLevel {
                base_level: 0,
                gamepad_curve: true,
            }),
        ]
        .into_iter()
        .collect(),
    }
}

fn caps_lock_trigger() -> Condition {
    let group = Group {
        logic: Logic::All,
        terms: [Term::Trigger {
            signal: Signal::Builtin(Builtin::CapsLock),
            duration_ms: 500,
        }]
        .into_iter()
        .collect(),
    };
    Condition {
        logic: Logic::Single,
        groups: [group].into_iter().collect(),
    }
}

fn lightbar_settings() -> lightbar::Settings {
    let indicator = Entry {
        condition: None,
        effect: lightbar::Effect::ProfileIndicator(ProfileIndicator {
            range: Range { first: 0, last: 9 },
            background: Background::Fixed(opaque(10, 10, 10)),
            segments: 5,
        }),
    };
    let meter = Entry {
        condition: Some(caps_lock_trigger()),
        effect: lightbar::Effect::Meter(Meter {
            signal: Signal::Builtin(Builtin::AnalogLevel),
            maximum: 255.0,
            color: opaque(0, 255, 0),
            background: None,
            gradient_start: Some(opaque(255, 0, 0)),
            range: Range { first: 2, last: 7 },
            reverse: true,
        }),
    };
    lightbar::Settings {
        enabled: true,
        entries: [indicator, meter].into_iter().collect(),
        use_global: true,
        brightness: Some(128),
    }
}

fn gamepad_settings() -> gamepad::Settings {
    let point = |travel, output| CurvePoint { travel, output };
    gamepad::Settings {
        curve: [
            point(10, 0),
            point(80, 60),
            point(160, 180),
            point(240, 255),
        ],
        normalization: Normalization::Square,
        single_direction_axes: true,
        left_stick_up_bias: Some(50),
    }
}

fn key_settings() -> keys::Settings {
    keys::Settings {
        actuation: Distance::from_fraction(0x2D2D),
        rapid_trigger_enabled: true,
        rapid_trigger: RapidTrigger {
            press: Distance::from_fraction(0x1000),
            release: None,
            continuous: false,
        },
        tap_duration: TapDuration::TwentyMilliseconds,
        scan_mode: ScanMode::Parallel,
        keyboard_in_gamepad_mode: true,
    }
}

fn switches() -> switches::Settings {
    let keys = (0..key_count())
        .map(|key| {
            (key == 3).then_some(Switch {
                model: 12,
                travel_offset: 4,
            })
        })
        .collect();
    switches::Settings {
        default: Some(Switch {
            model: 1,
            travel_offset: 0,
        }),
        keys,
    }
}

fn entries(entry: Entry) -> lightbar::Entries {
    [entry].into_iter().collect()
}

#[test]
fn keymaps_and_key_settings_survive_a_round_trip() {
    let mut keymap: keys::Keymap = filled(Action::None, key_count());
    keymap[0] = Action::Keyboard(0x29);
    keymap[1] = Action::Mouse(MouseButton::Back);
    keymap[2] = Action::System(SystemControl::Sleep);
    keymap[3] = Action::Gamepad(Control::LeftTrigger);
    keymap[4] = Action::Layer(Layer::Two);
    keymap[5] = Action::Function(Function::Profile(Slot::new(3)));
    let layer = LayerRef {
        profile: Slot::new(1),
        layer: Layer::One,
    };
    round_trip!(
        SetKeymap,
        LayerChange {
            profile: layer.profile,
            layer: layer.layer,
            value: keymap.clone(),
        },
        DONE
    );
    round_trip!(GetKeymap, layer, Ok(keymap));
    round_trip!(
        SetKeySettings,
        Change {
            profile: Slot::new(0),
            value: key_settings(),
        },
        DONE
    );
    round_trip!(
        SetKeyOverrides,
        Change {
            profile: Slot::new(0),
            value: filled(widest_override(), key_count()),
        },
        Err(Rejection::Storage)
    );
}

#[test]
fn advanced_keys_and_gamepad_settings_survive_a_round_trip() {
    let mod_tap = Behaviour::ModTap(ModTap {
        tap: Action::Keyboard(0x29),
        hold: Action::Keyboard(0xE0),
        hold_time_ms: ModTap::DEFAULT_HOLD_TIME_MS,
        interrupt: Interrupt::Hold,
    });
    let depth = Behaviour::DepthPriority(DepthPriority {
        other: Index::new(23),
        both_bottomed_out: Resolution::LastPressed,
    });
    let socd = Behaviour::Socd(Socd {
        other: Index::new(25),
        resolution: Resolution::Neither,
        both_when_bottomed_out: true,
    });
    let toggle = Behaviour::Toggle(Action::Keyboard(0x39));
    let advanced: advanced::Keys =
        core::iter::once(widest_advanced_key())
            .chain([(20, mod_tap), (21, toggle), (22, depth), (24, socd)].map(
                |(key, behaviour)| advanced::Key {
                    key: Index::new(key),
                    layer: Layer::Base,
                    behaviour,
                },
            ))
            .collect();
    round_trip!(
        SetAdvancedKeys,
        Change {
            profile: Slot::new(2),
            value: advanced.clone(),
        },
        DONE
    );
    round_trip!(GetAdvancedKeys, Slot::new(2), Ok(advanced));
    let mut bindings = filled(None, key_count());
    bindings[7] = Some(Control::HatUp);
    round_trip!(
        SetGamepadMapping,
        Change {
            profile: Slot::new(0),
            value: gamepad::Mapping {
                bindings,
                exclusive: true,
            },
        },
        DONE
    );
    round_trip!(
        SetGamepadSettings,
        Change {
            profile: Slot::new(0),
            value: gamepad_settings(),
        },
        DONE
    );
}

#[test]
fn lighting_and_light_bar_survive_a_round_trip() {
    round_trip!(
        SetLighting,
        Change {
            profile: Slot::new(3),
            value: lighting_settings(),
        },
        Err(Rejection::ProfileMissing(3))
    );
    round_trip!(
        SetKeyColors,
        LayerChange {
            profile: Slot::new(0),
            layer: Layer::Base,
            value: Some(filled(white(), key_count())),
        },
        DONE
    );
    round_trip!(
        GetKeyColors,
        LayerRef {
            profile: Slot::new(0),
            layer: Layer::Three,
        },
        Ok(None)
    );
    round_trip!(
        SetLightbar,
        Change {
            profile: Slot::new(0),
            value: lightbar_settings(),
        },
        DONE
    );
    round_trip!(GetLightbar, Slot::new(0), Ok(lightbar_settings()));
    round_trip!(SetGlobalLightbar, lightbar_settings().entries, DONE);
    let signals: lightbar::HostSignals = [HostSignal {
        id: 0xC0FF_EE00,
        value: 0.5,
    }]
    .into_iter()
    .collect();
    round_trip!(SetHostSignals, signals, DONE);
}

#[test]
fn keyboard_wide_settings_survive_a_round_trip() {
    round_trip!(SetSwitches, switches(), Err(Rejection::Locked));
    round_trip!(GetSwitches, (), Ok(switches()));
    round_trip!(
        SetDeviceSettings,
        device::Settings {
            mode: device::Mode::Xinput,
            keyboard: device::KeyboardFormat::Boot,
            speed: device::Speed::Full,
            gamepad_polling: device::PollingRate::Every125Microseconds,
        },
        DONE
    );
    round_trip!(SetLayout, Layout::Jis, DONE);
    round_trip!(
        SetGlobalSettings,
        GlobalSettings {
            idle: Some(Idle {
                dim_after_minutes: 5,
                off_after_minutes: 10,
                off_enabled: true,
            }),
            num_lock_inverted: true,
            release_rebound_filter: false,
        },
        DONE
    );
    round_trip!(
        SetProfileInfo,
        Change {
            profile: Slot::new(1),
            value: profile::Info {
                name: String::try_from("Rapid Profile").unwrap(),
                color: Rgb {
                    red: 255,
                    green: 40,
                    blue: 40,
                },
            },
        },
        DONE
    );
    round_trip!(GetCapabilities, (), Ok(capabilities()));
}

#[test]
fn the_largest_key_configuration_fits_one_frame() {
    let profile = Slot::new(u8::MAX);
    round_trip!(
        SetKeymap,
        LayerChange {
            profile,
            layer: Layer::Three,
            value: filled(widest_action(), MAX_KEYS),
        },
        DONE
    );
    round_trip!(
        SetKeyOverrides,
        Change {
            profile,
            value: filled(widest_override(), MAX_KEYS),
        },
        DONE
    );
    round_trip!(
        SetAdvancedKeys,
        Change {
            profile,
            value: filled(widest_advanced_key(), advanced::MAX_ENTRIES),
        },
        DONE
    );
    round_trip!(
        SetGamepadMapping,
        Change {
            profile,
            value: gamepad::Mapping {
                bindings: filled(Some(Control::HatRight), MAX_KEYS),
                exclusive: true,
            },
        },
        DONE
    );
    let widest_switch = Some(Switch {
        model: u8::MAX,
        travel_offset: u8::MAX,
    });
    round_trip!(
        SetSwitches,
        switches::Settings {
            default: widest_switch,
            keys: filled(widest_switch, MAX_KEYS),
        },
        DONE
    );
}

#[test]
fn the_largest_lighting_configuration_fits_one_frame() {
    let profile = Slot::new(u8::MAX);
    round_trip!(
        SetKeyColors,
        LayerChange {
            profile,
            layer: Layer::Three,
            value: Some(filled(white(), MAX_KEYS)),
        },
        DONE
    );
    let entries: lightbar::Entries = filled(widest_lightbar_entry(), lightbar::MAX_ENTRIES);
    round_trip!(
        SetLightbar,
        Change {
            profile,
            value: lightbar::Settings {
                enabled: true,
                entries: entries.clone(),
                use_global: true,
                brightness: Some(u8::MAX),
            },
        },
        DONE
    );
    round_trip!(SetGlobalLightbar, entries, DONE);
    let signals: lightbar::HostSignals = filled(
        HostSignal {
            id: u32::MAX,
            value: f32::MAX,
        },
        lightbar::MAX_HOST_SIGNALS,
    );
    round_trip!(SetHostSignals, signals, DONE);
    let mut name = String::new();
    while name.push('\u{20AC}').is_ok() {}
    round_trip!(
        SetProfileInfo,
        Change {
            profile,
            value: profile::Info {
                name,
                color: Rgb::default(),
            },
        },
        DONE
    );
}

#[test]
fn keymaps_must_cover_the_key_matrix_with_actions_the_reports_can_carry() {
    let capabilities = capabilities();
    let short: keys::Keymap = filled(Action::None, key_count() - 1);
    assert_eq!(
        short.validate(&capabilities),
        Err(Invalid::KeyCount {
            expected: 102,
            found: 101
        })
    );
    for (action, invalid) in [
        (Action::Keyboard(0x00), Invalid::KeyboardUsage(0x00)),
        (Action::Keyboard(0xE8), Invalid::KeyboardUsage(0xE8)),
        (Action::Consumer(0), Invalid::ConsumerUsage(0)),
        (Action::Consumer(0x02A3), Invalid::ConsumerUsage(0x02A3)),
        (Action::Layer(Layer::Base), Invalid::MomentaryBaseLayer),
        (
            Action::Function(Function::Profile(Slot::new(4))),
            Invalid::ProfileOutsideRange(4),
        ),
    ] {
        let mut keymap: keys::Keymap = filled(Action::None, key_count());
        keymap[50] = action;
        assert_eq!(keymap.validate(&capabilities), Err(invalid), "{action:?}");
    }
    let change = LayerChange {
        profile: Slot::new(4),
        layer: Layer::Base,
        value: filled::<Action, MAX_KEYS>(Action::None, key_count()),
    };
    assert_eq!(
        change.validate(&capabilities),
        Err(Invalid::ProfileOutsideRange(4))
    );
}

#[test]
fn distances_of_zero_and_broken_advanced_keys_are_refused() {
    let capabilities = capabilities();
    let mut overrides: keys::Overrides = filled(Override::default(), key_count());
    overrides[9].rapid_trigger = RapidTriggerOverride::With(RapidTrigger {
        press: Distance::from_fraction(100),
        release: Some(Distance::from_fraction(0)),
        continuous: false,
    });
    assert_eq!(
        overrides.validate(&capabilities),
        Err(Invalid::ZeroDistance)
    );
    let mut settings = key_settings();
    settings.actuation = Distance::from_fraction(0);
    assert_eq!(settings.validate(&capabilities), Err(Invalid::ZeroDistance));

    let pair = |key, other| advanced::Key {
        key: Index::new(key),
        layer: Layer::Base,
        behaviour: Behaviour::Socd(Socd {
            other: Index::new(other),
            resolution: Resolution::LastPressed,
            both_when_bottomed_out: false,
        }),
    };
    let instant_mod_tap = advanced::Key {
        key: Index::new(1),
        layer: Layer::Base,
        behaviour: Behaviour::ModTap(ModTap {
            tap: Action::Keyboard(0x04),
            hold: Action::Keyboard(0xE1),
            hold_time_ms: 0,
            interrupt: Interrupt::Auto,
        }),
    };
    for (key, invalid) in [
        (pair(5, 5), Invalid::KeyPairedWithItself(5)),
        (pair(102, 5), Invalid::KeyOutsideMatrix(102)),
        (pair(5, 200), Invalid::KeyOutsideMatrix(200)),
        (instant_mod_tap, Invalid::ZeroHoldTime),
    ] {
        let keys: advanced::Keys = [key].into_iter().collect();
        assert_eq!(keys.validate(&capabilities), Err(invalid));
    }
}

#[test]
fn gamepad_curves_must_rise_and_effects_keep_the_c_rate_limits() {
    let capabilities = capabilities();
    let mut curve = gamepad_settings();
    curve.curve[2].travel = 70;
    assert_eq!(curve.validate(&capabilities), Err(Invalid::CurveNotRising));
    let mut bias = gamepad_settings();
    bias.left_stick_up_bias = Some(101);
    assert_eq!(bias.validate(&capabilities), Err(Invalid::Percentage(101)));

    let mut lighting = lighting_settings();
    lighting.effects[0] = Effect::Scanner(Scanner {
        width: 2,
        steps_per_second: 61,
        outside_level: 0,
        direction: ScannerDirection::Up,
    });
    assert_eq!(
        lighting.validate(&capabilities),
        Err(Invalid::Rate {
            value: 61,
            min: 1,
            max: 60
        })
    );
    lighting.effects[0] = Effect::HueCycle(HueCycle {
        hues: HueRange::From2To39,
        period: 0,
    });
    assert_eq!(
        lighting.validate(&capabilities),
        Err(Invalid::Rate {
            value: 0,
            min: 1,
            max: 255
        })
    );
}

#[test]
fn light_bar_entries_stay_on_the_light_bar_and_the_key_matrix() {
    let capabilities = capabilities();
    let blink = |target| Entry {
        condition: None,
        effect: lightbar::Effect::Blink(Blink {
            from: Color::TRANSPARENT,
            to: opaque(1, 2, 3),
            period_ms: 400,
            waveform: Waveform::Triangle,
            target,
        }),
    };
    assert_eq!(
        entries(blink(Target::Lightbar(Range { first: 3, last: 10 }))).validate(&capabilities),
        Err(Invalid::LightbarRange {
            first: 3,
            last: 10,
            leds: 10
        })
    );
    let rect = KeyRect {
        first: Position { row: 0, column: 0 },
        last: Position { row: 6, column: 0 },
    };
    assert_eq!(
        entries(blink(Target::Keys(rect))).validate(&capabilities),
        Err(Invalid::PositionOutsideMatrix { row: 6, column: 0 })
    );
    let mut meter = lightbar_settings();
    if let lightbar::Effect::Meter(meter) = &mut meter.entries[1].effect {
        meter.maximum = f32::NAN;
    }
    assert_eq!(meter.validate(&capabilities), Err(Invalid::MeterMaximum));
}

#[test]
fn host_signals_and_light_bars_need_a_keyboard_that_has_them() {
    let mut capabilities = capabilities();
    let host_condition = Entry {
        condition: Some(Condition {
            logic: Logic::Single,
            groups: [Group {
                logic: Logic::Single,
                terms: [Term::Compare(Comparison {
                    signal: Signal::Host(7),
                    comparator: Comparator::Greater,
                    operand: Operand::Constant(0.5),
                    hold_ms: None,
                })]
                .into_iter()
                .collect(),
            }]
            .into_iter()
            .collect(),
        }),
        effect: lightbar::Effect::Solid(Solid {
            color: opaque(9, 9, 9),
            target: Target::Lightbar(Range { first: 0, last: 0 }),
        }),
    };
    assert_eq!(
        entries(host_condition.clone()).validate(&capabilities),
        Ok(())
    );
    capabilities.host_signals = false;
    assert_eq!(
        entries(host_condition.clone()).validate(&capabilities),
        Err(Invalid::NoHostSignals)
    );
    capabilities.lightbar_leds = 0;
    assert_eq!(
        entries(host_condition).validate(&capabilities),
        Err(Invalid::NoLightbar)
    );
}

#[test]
fn switch_models_and_layouts_must_exist_on_the_keyboard() {
    let mut capabilities = capabilities();
    let mut unknown = switches();
    unknown.keys[0] = Some(Switch {
        model: 49,
        travel_offset: 0,
    });
    assert_eq!(
        unknown.validate(&capabilities),
        Err(Invalid::SwitchModel(49))
    );
    capabilities.layouts = [Layout::Ansi, Layout::Iso].into_iter().collect();
    assert_eq!(
        Layout::Jis.validate(&capabilities),
        Err(Invalid::Layout(Layout::Jis))
    );
}

#[test]
fn rejections_read_as_one_lowercase_sentence() {
    assert_eq!(
        Rejection::from(Invalid::KeyboardUsage(0xE8)).to_string(),
        "keyboard usage `0xe8` has no place in the keyboard report"
    );
    assert_eq!(
        Rejection::ProfileMissing(3).to_string(),
        "profile 3 does not exist"
    );
    assert_eq!(
        Invalid::LightbarRange {
            first: 3,
            last: 10,
            leds: 10
        }
        .to_string(),
        "light bar positions 3..=10 lie outside the 10 LEDs"
    );
}

const PINNED_KEYS: &[(&str, u64, u64)] = &[
    (
        "openwoot/version",
        0xdc7f_f85d_7880_40a4,
        0xe671_7cee_671e_7dae,
    ),
    (
        "openwoot/update-mode",
        0xcd60_8649_8d23_c62d,
        0xcd60_8649_8d23_c62d,
    ),
    (
        "openwoot/capabilities",
        0xb425_ca7d_7ee1_f14b,
        0xfc7a_ee30_d302_0700,
    ),
    (
        "openwoot/device",
        0x20f1_5e34_a504_9906,
        0x9347_3218_2614_68ac,
    ),
    (
        "openwoot/device/set",
        0x9989_0643_a008_937f,
        0x10d6_e6dd_06f8_d6f6,
    ),
    (
        "openwoot/layout",
        0x5cf8_b683_4b68_cff1,
        0x6408_06b7_a9dd_b853,
    ),
    (
        "openwoot/layout/set",
        0x5252_9d11_09fb_e750,
        0x7cd7_d6df_7fc4_2097,
    ),
    (
        "openwoot/global",
        0x5f97_9749_9161_c008,
        0x3b4e_9d52_96d2_f2de,
    ),
    (
        "openwoot/global/set",
        0x910c_485d_d296_a402,
        0x7f9d_bf11_2b3f_3b48,
    ),
    (
        "openwoot/profiles",
        0x84d8_7b52_264c_95b2,
        0x433a_4f8e_c88f_0a5e,
    ),
    (
        "openwoot/profiles/create",
        0x8d00_db21_a224_092b,
        0x8f78_f4ee_606a_f8da,
    ),
    (
        "openwoot/profiles/delete",
        0x4912_6876_6294_b0fa,
        0x55c7_7f7f_acf1_9660,
    ),
    (
        "openwoot/profiles/activate",
        0x37c9_33d9_65d3_1a21,
        0x1f52_605b_2386_6084,
    ),
    (
        "openwoot/profile/info",
        0xbdc2_45a4_1cc8_380b,
        0x3471_efb2_6b2d_a887,
    ),
    (
        "openwoot/profile/info/set",
        0x630d_2022_8625_4869,
        0xb4af_5bf1_6882_aa61,
    ),
    (
        "openwoot/profile/revision",
        0xac4e_739d_ec0c_48a7,
        0xc684_43a0_e917_dc82,
    ),
    (
        "openwoot/profile/keys",
        0x813d_9b84_5006_f39c,
        0x9d83_07ea_f1b3_bf46,
    ),
    (
        "openwoot/profile/keys/set",
        0x0ec8_7477_34b1_0eec,
        0xb834_1c6e_923a_a41d,
    ),
    (
        "openwoot/profile/keymap",
        0x55a9_27a1_0b2b_6414,
        0x7881_81d7_dc93_5afb,
    ),
    (
        "openwoot/profile/keymap/set",
        0xae71_d018_7915_3dc1,
        0xf1fe_5dc9_a1c7_40ac,
    ),
    (
        "openwoot/profile/overrides",
        0x523d_cb34_363c_e7a1,
        0xdfed_fe47_e321_d572,
    ),
    (
        "openwoot/profile/overrides/set",
        0xaa57_2ab4_f1ec_b6ef,
        0xe559_30ba_0f85_3fc8,
    ),
    (
        "openwoot/profile/advanced",
        0x5b3a_106c_0962_78ef,
        0xc320_2671_2314_7dcc,
    ),
    (
        "openwoot/profile/advanced/set",
        0x2080_ee3e_0588_660d,
        0x4e48_7589_a5c5_7eec,
    ),
    (
        "openwoot/profile/gamepad/mapping",
        0x2599_23c4_84b2_47c4,
        0xa457_6648_a1b5_7a58,
    ),
    (
        "openwoot/profile/gamepad/mapping/set",
        0x9b1e_f126_5a58_83d6,
        0x6c62_a74c_3ece_a400,
    ),
    (
        "openwoot/profile/gamepad",
        0x98d7_529b_3cc1_f64d,
        0xe5fc_02e0_b3c0_369e,
    ),
    (
        "openwoot/profile/gamepad/set",
        0xace3_bdad_a958_c2a4,
        0xd764_2073_4bb4_5322,
    ),
    (
        "openwoot/profile/lighting",
        0x3907_7288_a0ac_1c42,
        0xb034_1baf_9ec3_055f,
    ),
    (
        "openwoot/profile/lighting/set",
        0xb7dc_c836_ea92_fbac,
        0x407f_71f7_3aa6_0ab4,
    ),
    (
        "openwoot/profile/colors",
        0x70a8_2f84_da9b_4cc8,
        0xeaed_2a79_5f80_94b1,
    ),
    (
        "openwoot/profile/colors/set",
        0x1a93_a71d_be21_956d,
        0xc018_4de0_38c2_8f1e,
    ),
    (
        "openwoot/profile/lightbar",
        0xd499_40fd_e0af_77dc,
        0xbb7d_9e31_a04f_f85c,
    ),
    (
        "openwoot/profile/lightbar/set",
        0x6a55_a53f_fe44_5ccd,
        0x93f6_e4b0_a59f_6e0a,
    ),
    (
        "openwoot/lightbar",
        0xe16e_d627_9743_b3d3,
        0xe39e_0c23_fb3b_651f,
    ),
    (
        "openwoot/lightbar/set",
        0x8d5a_2756_4ab5_bfc2,
        0xcd8a_76ab_4f26_af0d,
    ),
    (
        "openwoot/signals",
        0xcfbf_d99f_cdf8_95ab,
        0x1d43_0438_3a93_0652,
    ),
    (
        "openwoot/signals/set",
        0x2d23_a5a4_1d0e_db42,
        0x6f84_9135_83c3_358f,
    ),
    (
        "openwoot/switches",
        0x7aca_b366_3901_7699,
        0x73b8_1f29_260c_0f4c,
    ),
    (
        "openwoot/switches/set",
        0x53f3_55ef_5bfe_8fe1,
        0x6681_9c2e_97b2_856a,
    ),
    (
        "openwoot/switches/models",
        0xb907_0ae4_caed_dd1b,
        0xbef9_06c9_21c8_bcea,
    ),
    (
        "openwoot/save",
        0xc14f_75e9_d701_b350,
        0x7cb6_efc6_54c3_4d37,
    ),
    (
        "openwoot/revert",
        0xf052_f692_ba8d_bed4,
        0x0552_ce39_7d26_a893,
    ),
    (
        "openwoot/factory-reset",
        0xdc37_0a9f_6587_1c6e,
        0x295f_0d5e_0f1d_2782,
    ),
];

#[test]
fn endpoint_keys_are_pinned_so_a_changed_type_needs_a_new_path() {
    let keys: std::vec::Vec<_> = ENDPOINT_LIST
        .endpoints
        .iter()
        .filter(|(path, ..)| path.starts_with("openwoot/"))
        .map(|(path, request, response)| {
            (
                *path,
                u64::from_be_bytes(request.to_bytes()),
                u64::from_be_bytes(response.to_bytes()),
            )
        })
        .collect();
    assert_eq!(keys, PINNED_KEYS);
}

#[test]
fn requests_and_responses_of_different_endpoints_never_share_a_key() {
    for keys in [
        ENDPOINT_LIST
            .endpoints
            .iter()
            .map(|(_, request, _)| request.to_bytes())
            .collect::<std::vec::Vec<_>>(),
        ENDPOINT_LIST
            .endpoints
            .iter()
            .map(|(_, _, response)| response.to_bytes())
            .collect(),
    ] {
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), keys.len());
    }
}
