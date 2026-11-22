use openwoot_protocol::{
    advanced::{self, Behaviour, Interrupt, ModTap, Resolution, Socd},
    configuration::{Invalid, MAX_KEYS, Rejection, Validate},
    device::Layout,
    keys::{
        self, Action, Distance, Function, Index, Layer, Override, Position, RapidTrigger,
        RapidTriggerOverride,
    },
    lightbar::{
        self, Blink, Comparator, Comparison, Condition, Entry, Group, KeyRect, Logic, Operand,
        Range, Signal, Solid, Target, Term, Waveform,
    },
    lighting::{Color, Effect, HueCycle, HueRange, Scanner, ScannerDirection},
    profile::{LayerChange, Slot},
    switches::Switch,
};

use crate::support::{
    capabilities, filled, gamepad_settings, key_count, key_settings, lightbar_settings,
    lighting_settings, opaque, switches,
};

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

fn entries(entry: Entry) -> lightbar::Entries {
    [entry].into_iter().collect()
}
