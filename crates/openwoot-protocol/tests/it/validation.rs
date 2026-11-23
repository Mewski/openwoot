use openwoot_protocol::{
    advanced::{self, Behaviour, Interrupt, ModTap, Resolution, Socd},
    configuration::{Invalid, MAX_KEYS, Rejection, Validate},
    device::Layout,
    keys::{
        self, Action, Distance, Function, Index, Layer, Override, Position, RapidTrigger,
        RapidTriggerOverride,
    },
    lightbar::{
        self, Background, Blink, Builtin, Comparator, Comparison, Condition, Custom, Entry, Group,
        KeyRect, Logic, Meter, Operand, ProfileIndicator, Range, Signal, Solid, Target, Term,
        Waveform,
    },
    lighting::{Color, Effect, HueCycle, HueRange, Scanner, ScannerDirection},
    profile::{LayerChange, Slot},
    switches::{self, MAX_TUNED, Switch},
};

use crate::support::{
    capabilities, filled, gamepad_settings, key_count, key_settings, lightbar_settings,
    lighting_settings, opaque, switches, white,
};

#[test]
fn keymaps_must_cover_the_key_matrix_with_actions_the_reports_can_carry() {
    let capabilities = capabilities();
    let short: keys::Keymap = filled(Action::None, key_count() - 1);
    assert_eq!(
        short.validate(&capabilities),
        Err(Invalid::KeyCountDiffersFromMatrix {
            expected: 102,
            found: 101
        })
    );
    for (action, invalid) in [
        (
            Action::Keyboard(0x00),
            Invalid::KeyboardUsageOutsideReport(0x00),
        ),
        (
            Action::Keyboard(0xE8),
            Invalid::KeyboardUsageOutsideReport(0xE8),
        ),
        (
            Action::Keyboard(0x32),
            Invalid::KeyboardUsageOutsideReport(0x32),
        ),
        (
            Action::Keyboard(0x9C),
            Invalid::KeyboardUsageOutsideReport(0x9C),
        ),
        (
            Action::Keyboard(0xA5),
            Invalid::KeyboardUsageOutsideReport(0xA5),
        ),
        (
            Action::Keyboard(0xAF),
            Invalid::KeyboardUsageOutsideReport(0xAF),
        ),
        (Action::Consumer(0), Invalid::ConsumerUsageOutsideRange(0)),
        (
            Action::Consumer(0x02A3),
            Invalid::ConsumerUsageOutsideRange(0x02A3),
        ),
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
fn gamepad_curves_must_rise_and_effects_keep_their_rate_limits() {
    let capabilities = capabilities();
    let mut curve = gamepad_settings();
    curve.curve[2].travel = 70;
    assert_eq!(curve.validate(&capabilities), Err(Invalid::CurveNotRising));
    let mut bias = gamepad_settings();
    bias.left_stick_up_bias = Some(101);
    assert_eq!(
        bias.validate(&capabilities),
        Err(Invalid::PercentageAboveHundred(101))
    );

    let mut lighting = lighting_settings();
    lighting.effects[0] = Effect::Scanner(Scanner {
        width: 2,
        steps_per_second: 61,
        outside_level: 0,
        direction: ScannerDirection::Up,
    });
    assert_eq!(
        lighting.validate(&capabilities),
        Err(Invalid::RateOutsideRange {
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
        Err(Invalid::RateOutsideRange {
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
        Err(Invalid::LightbarRangeOutsideLeds {
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
    assert_eq!(
        meter.validate(&capabilities),
        Err(Invalid::MeterMaximumNotPositive)
    );
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
        Err(Invalid::SwitchModelUnknown(49))
    );
    capabilities.layouts = [Layout::Ansi, Layout::Iso].into_iter().collect();
    assert_eq!(
        Layout::Jis.validate(&capabilities),
        Err(Invalid::LayoutMissing(Layout::Jis))
    );
}

fn models() -> switches::Models {
    [
        switches::Model {
            travel: 80,
            group: 0,
        },
        switches::Model {
            travel: 40,
            group: 3,
        },
    ]
    .into_iter()
    .collect()
}

fn set_on_key(model: u8, travel_offset: u8) -> switches::Settings {
    let mut settings = switches();
    settings.default = None;
    if let Some(key) = settings.keys.get_mut(3) {
        *key = Some(Switch {
            model,
            travel_offset,
        });
    }
    settings
}

#[test]
fn a_travel_offset_must_stay_below_the_travel_of_its_model() {
    let models = models();
    assert_eq!(set_on_key(0, 79).check_models(&models), Ok(()));
    assert_eq!(set_on_key(1, 39).check_models(&models), Ok(()));
    assert_eq!(
        set_on_key(1, 40).check_models(&models),
        Err(Invalid::TravelOffsetReachesTravel {
            model: 1,
            offset: 40,
            travel: 40
        })
    );
    assert_eq!(
        set_on_key(0, u8::MAX).check_models(&models),
        Err(Invalid::TravelOffsetReachesTravel {
            model: 0,
            offset: u8::MAX,
            travel: 80
        })
    );
    let default_beyond = switches::Settings {
        default: Some(Switch {
            model: 1,
            travel_offset: 41,
        }),
        ..set_on_key(0, 0)
    };
    assert_eq!(
        default_beyond.check_models(&models),
        Err(Invalid::TravelOffsetReachesTravel {
            model: 1,
            offset: 41,
            travel: 40
        })
    );
    assert_eq!(
        set_on_key(2, 0).check_models(&models),
        Err(Invalid::SwitchModelUnknown(2))
    );
}

#[test]
fn at_most_the_limit_of_distinct_switches_carry_a_travel_offset() {
    let models = models();
    let mut settings = set_on_key(0, 0);
    let limit = u8::try_from(MAX_TUNED).unwrap();
    for (offset, key) in (1..=limit).zip(settings.keys.iter_mut()) {
        *key = Some(Switch {
            model: 0,
            travel_offset: offset,
        });
    }
    settings.default = Some(Switch {
        model: 0,
        travel_offset: 1,
    });
    let repeated = settings.keys.len() - 1;
    settings.keys[repeated] = Some(Switch {
        model: 0,
        travel_offset: 0,
    });
    assert_eq!(settings.check_models(&models), Ok(()));
    settings.keys[repeated] = Some(Switch {
        model: 1,
        travel_offset: 1,
    });
    assert_eq!(
        settings.check_models(&models),
        Err(Invalid::TunedSwitchesExceedLimit)
    );
}

#[test]
fn a_key_without_its_own_switch_uses_the_default() {
    let settings = switches();
    assert_eq!(
        settings.of_key(3),
        Some(Switch {
            model: 12,
            travel_offset: 4
        })
    );
    assert_eq!(settings.of_key(0), settings.default);
    assert_eq!(settings.of_key(MAX_KEYS), settings.default);
    let bare = switches::Settings {
        default: None,
        ..switches()
    };
    assert_eq!(bare.of_key(0), None);
    assert_eq!(bare.switches().count(), 1);
    assert_eq!(settings.switches().count(), 2);
}

#[test]
fn rejections_read_as_one_lowercase_sentence() {
    assert_eq!(
        Invalid::TravelOffsetReachesTravel {
            model: 2,
            offset: 66,
            travel: 66
        }
        .to_string(),
        "travel offset 66 reaches the travel 66 of switch model 2"
    );
    assert_eq!(
        Invalid::TunedSwitchesExceedLimit.to_string(),
        "more than 16 distinct switches have a travel offset"
    );
    assert_eq!(
        Rejection::from(Invalid::KeyboardUsageOutsideReport(0xE8)).to_string(),
        "keyboard usage `0xe8` has no place in the keyboard report"
    );
    assert_eq!(
        Rejection::ProfileMissing(3).to_string(),
        "profile 3 does not exist"
    );
    assert_eq!(
        Invalid::LightbarRangeOutsideLeds {
            first: 3,
            last: 10,
            leds: 10
        }
        .to_string(),
        "light bar positions 3..=10 lie outside the 10 LEDs"
    );
}

#[test]
fn a_key_takes_one_advanced_key_per_layer() {
    let capabilities = capabilities();
    let toggle = |key, layer| advanced::Key {
        key: Index::new(key),
        layer,
        behaviour: Behaviour::Toggle(Action::Keyboard(0x39)),
    };
    let pair = |key, other| advanced::Key {
        key: Index::new(key),
        layer: Layer::Base,
        behaviour: Behaviour::Socd(Socd {
            other: Index::new(other),
            resolution: Resolution::LastPressed,
            both_when_bottomed_out: false,
        }),
    };
    let layered: advanced::Keys = [toggle(5, Layer::Base), toggle(5, Layer::One), pair(6, 7)]
        .into_iter()
        .collect();
    assert_eq!(layered.validate(&capabilities), Ok(()));
    for (keys, key) in [
        ([toggle(5, Layer::Base), toggle(5, Layer::Base)], 5),
        ([pair(6, 7), toggle(7, Layer::Base)], 7),
        ([pair(6, 7), pair(7, 8)], 7),
    ] {
        let keys: advanced::Keys = keys.into_iter().collect();
        assert_eq!(
            keys.validate(&capabilities),
            Err(Invalid::KeyBoundTwice(key))
        );
    }
}

#[test]
fn light_bar_terms_need_a_duration_the_keyboard_can_count() {
    let capabilities = capabilities();
    let trigger = |duration_ms| Term::Trigger {
        signal: Signal::Builtin(Builtin::CapsLock),
        duration_ms,
    };
    let compare = |hold_ms| {
        Term::Compare(Comparison {
            signal: Signal::Builtin(Builtin::KeyRate),
            comparator: Comparator::Greater,
            operand: Operand::Constant(0.5),
            hold_ms,
        })
    };
    for (term, verdict) in [
        (trigger(500), Ok(())),
        (trigger(60_000), Ok(())),
        (trigger(0), Err(Invalid::ZeroDuration)),
        (trigger(60_001), Err(Invalid::HoldTimeTooLong(60_001))),
        (compare(None), Ok(())),
        (compare(Some(60_000)), Ok(())),
        (compare(Some(0)), Err(Invalid::ZeroHoldTime)),
        (compare(Some(60_001)), Err(Invalid::HoldTimeTooLong(60_001))),
        (compare(Some(65_536)), Err(Invalid::HoldTimeTooLong(65_536))),
    ] {
        let condition = Condition {
            logic: Logic::Single,
            groups: filled(
                Group {
                    logic: Logic::Single,
                    terms: filled(term, 1),
                },
                1,
            ),
        };
        assert_eq!(
            entries(shown_when(condition)).validate(&capabilities),
            verdict,
            "{term:?}"
        );
    }
}

#[test]
fn light_bar_conditions_that_can_never_hold_are_refused() {
    let capabilities = capabilities();
    let term = Term::Trigger {
        signal: Signal::Builtin(Builtin::CapsLock),
        duration_ms: 500,
    };
    let shaped = |logic, groups: &[(Logic, usize)]| Condition {
        logic,
        groups: groups
            .iter()
            .map(|&(logic, terms)| Group {
                logic,
                terms: filled(term, terms),
            })
            .collect(),
    };
    for (condition, verdict) in [
        (shaped(Logic::All, &[]), Ok(())),
        (shaped(Logic::Any, &[]), Err(Invalid::ConditionNeverHolds)),
        (
            shaped(Logic::Single, &[]),
            Err(Invalid::ConditionNeverHolds),
        ),
        (
            shaped(Logic::Single, &[(Logic::All, 1), (Logic::All, 1)]),
            Err(Invalid::ConditionNeverHolds),
        ),
        (
            shaped(Logic::Single, &[(Logic::Single, 2)]),
            Err(Invalid::ConditionNeverHolds),
        ),
        (
            shaped(Logic::All, &[(Logic::Any, 0)]),
            Err(Invalid::ConditionNeverHolds),
        ),
        (
            shaped(Logic::Any, &[(Logic::Any, 0), (Logic::All, 2)]),
            Ok(()),
        ),
        (
            shaped(Logic::All, &[(Logic::Single, 1), (Logic::Any, 2)]),
            Ok(()),
        ),
    ] {
        assert_eq!(
            entries(shown_when(condition.clone())).validate(&capabilities),
            verdict,
            "{condition:?}"
        );
    }
}

#[test]
fn key_rectangles_run_from_their_first_key_to_their_last() {
    let capabilities = capabilities();
    let lit = |first, last| {
        entries(Entry {
            condition: None,
            effect: lightbar::Effect::Solid(Solid {
                color: opaque(1, 2, 3),
                target: Target::Keys(rect(first, last)),
            }),
        })
    };
    assert_eq!(lit((1, 2), (1, 2)).validate(&capabilities), Ok(()));
    assert_eq!(lit((0, 0), (5, 16)).validate(&capabilities), Ok(()));
    for (first, last) in [((2, 0), (1, 0)), ((0, 3), (0, 2)), ((5, 16), (0, 0))] {
        assert_eq!(
            lit(first, last).validate(&capabilities),
            Err(Invalid::KeyRectInverted)
        );
    }
}

#[test]
fn every_light_bar_entry_may_watch_one_signal_with_every_term() {
    let capabilities = capabilities();
    let rate = Term::Compare(Comparison {
        signal: Signal::Builtin(Builtin::KeyRate),
        comparator: Comparator::Greater,
        operand: Operand::Signal(Signal::Builtin(Builtin::KeyRate)),
        hold_ms: None,
    });
    let watching = shown_when(Condition {
        logic: Logic::All,
        groups: filled(
            Group {
                logic: Logic::All,
                terms: filled(rate, lightbar::MAX_TERMS),
            },
            lightbar::MAX_GROUPS,
        ),
    });
    let full: lightbar::Entries = filled(watching, lightbar::MAX_ENTRIES);
    assert_eq!(full.validate(&capabilities), Ok(()));
}

#[test]
fn light_bar_entries_keep_to_the_host_signals_the_keyboard_can_list() {
    let capabilities = capabilities();
    let mut id = 0_u32;
    let mut host = || {
        id += 1;
        Signal::Host(id)
    };
    let mut entry = || {
        let mut term = || {
            Term::Compare(Comparison {
                signal: host(),
                comparator: Comparator::Greater,
                operand: Operand::Signal(host()),
                hold_ms: None,
            })
        };
        let mut group = || Group {
            logic: Logic::All,
            terms: [term(), term()].into_iter().collect(),
        };
        Entry {
            condition: Some(Condition {
                logic: Logic::All,
                groups: [group(), group()].into_iter().collect(),
            }),
            effect: lightbar::Effect::Meter(Meter {
                signal: host(),
                maximum: 1.0,
                color: Color::TRANSPARENT,
                background: None,
                gradient_start: None,
                range: Range { first: 0, last: 9 },
                reverse: false,
            }),
        }
    };
    let listed: lightbar::Entries = [entry(), entry()].into_iter().collect();
    assert_eq!(listed.validate(&capabilities), Ok(()));
    let crowded: lightbar::Entries = [entry(), entry(), entry()].into_iter().collect();
    assert_eq!(
        crowded.validate(&capabilities),
        Err(Invalid::LightbarHostSignalsExceedLimit)
    );
}

#[test]
fn light_bar_effects_need_a_period_segments_and_colours_that_fit() {
    let capabilities = capabilities();
    let drawn = |effect| {
        entries(Entry {
            condition: None,
            effect,
        })
    };
    let whole = Range { first: 0, last: 9 };
    assert_eq!(
        drawn(lightbar::Effect::Blink(Blink {
            from: Color::TRANSPARENT,
            to: opaque(1, 2, 3),
            period_ms: 0,
            waveform: Waveform::Square,
            target: Target::Lightbar(whole),
        }))
        .validate(&capabilities),
        Err(Invalid::ZeroPeriod)
    );
    assert_eq!(
        drawn(lightbar::Effect::ProfileIndicator(ProfileIndicator {
            range: whole,
            background: Background::Transparent,
            segments: 0,
        }))
        .validate(&capabilities),
        Err(Invalid::ZeroSegments)
    );
    assert_eq!(
        drawn(lightbar::Effect::Custom(Custom {
            colors: filled(white(), 11),
            range: whole,
        }))
        .validate(&capabilities),
        Err(Invalid::LightbarColorsExceedLeds {
            count: 11,
            leds: 10
        })
    );
}

fn entries(entry: Entry) -> lightbar::Entries {
    [entry].into_iter().collect()
}

fn shown_when(condition: Condition) -> Entry {
    Entry {
        condition: Some(condition),
        effect: lightbar::Effect::Solid(Solid {
            color: opaque(9, 9, 9),
            target: Target::Lightbar(Range { first: 0, last: 0 }),
        }),
    }
}

fn rect((first_row, first_column): (u8, u8), (last_row, last_column): (u8, u8)) -> KeyRect {
    KeyRect {
        first: Position {
            row: first_row,
            column: first_column,
        },
        last: Position {
            row: last_row,
            column: last_column,
        },
    }
}

#[test]
fn per_key_lists_host_colours_and_profile_changes_check_the_keyboard() {
    use openwoot_protocol::{
        gamepad, host_lighting::KeyColor, lightbar::HostSignal, lighting::KeyColors,
        profile::Change,
    };

    let mut capabilities = capabilities();
    let mapping = gamepad::Mapping {
        bindings: filled(None, key_count() + 1),
        exclusive: false,
    };
    assert_eq!(
        mapping.validate(&capabilities),
        Err(Invalid::KeyCountDiffersFromMatrix {
            expected: 102,
            found: 103
        })
    );
    let colors: KeyColors = filled(white(), key_count() - 2);
    assert_eq!(
        colors.validate(&capabilities),
        Err(Invalid::KeyCountDiffersFromMatrix {
            expected: 102,
            found: 100
        })
    );
    let host_color = |row, column| KeyColor {
        key: Position { row, column },
        color: white(),
    };
    assert_eq!(host_color(5, 16).validate(&capabilities), Ok(()));
    assert_eq!(
        host_color(5, 17).validate(&capabilities),
        Err(Invalid::PositionOutsideMatrix { row: 5, column: 17 })
    );
    let change = |profile| Change {
        profile: Slot::new(profile),
        value: key_settings(),
    };
    assert_eq!(change(3).validate(&capabilities), Ok(()));
    assert_eq!(
        change(4).validate(&capabilities),
        Err(Invalid::ProfileOutsideRange(4))
    );
    let mut flat = gamepad_settings();
    for point in &mut flat.curve {
        point.travel = 40;
    }
    assert_eq!(flat.validate(&capabilities), Err(Invalid::CurveNotRising));
    let signals: lightbar::HostSignals = filled(HostSignal { id: 1, value: 0.5 }, 1);
    assert_eq!(signals.validate(&capabilities), Ok(()));
    capabilities.host_signals = false;
    assert_eq!(signals.validate(&capabilities), Err(Invalid::NoHostSignals));
}
