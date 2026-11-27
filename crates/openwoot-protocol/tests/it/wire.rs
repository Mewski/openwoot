use core::fmt::Debug;

use openwoot_protocol::{
    advanced::{self, Behaviour, DepthPriority, Interrupt, ModTap, Resolution, Socd},
    apps::{self, App, Filter, Link, Matching, Text},
    configuration::{Invalid, Rejection},
    device::{self, KeyboardFormat, Layout, Mode, PollingRate, Speed},
    diagnostics::{self, BusError},
    gamepad::{self, Control, Player},
    geometry::{self, Shape},
    heapless::{String, Vec},
    keys::{
        self, Action, Distance, Function, Index, Layer, MouseButton, Override, RapidTrigger,
        RapidTriggerOverride, SystemControl,
    },
    lightbar::{
        self, Background, Blink, Builtin, Comparator, Comparison, Effect, KeyRect, Mirror, Operand,
        Range, Signal, Solid, Target, Term, Waveform,
    },
    lighting::{self, HueRange, ReactiveColoring},
    macros::{self, Encoded, Macro, Summary},
    profile::{self, Namespace, Slot},
    sensors::Reading,
    switches,
    wire::{Decode, Encode},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::support::{
    agrees_with_postcard, decodes_like_postcard, gamepad_settings, key_settings, largest_macro,
    lightbar_settings, lighting_settings, mutations, opaque, sample_macro, switches, white,
    widest_action, widest_advanced_key, widest_lightbar_entry, widest_override, wire_bytes,
};

#[test]
fn every_rejection_encodes_as_postcard_does() {
    let invalid = [
        Invalid::KeyCountDiffersFromMatrix {
            expected: 102,
            found: 300,
        },
        Invalid::KeyOutsideMatrix(200),
        Invalid::PositionOutsideMatrix { row: 7, column: 18 },
        Invalid::KeyPairedWithItself(3),
        Invalid::KeyBoundTwice(4),
        Invalid::KeyboardUsageOutsideReport(0xFF),
        Invalid::ConsumerUsageOutsideRange(0xFFFF),
        Invalid::ProfileOutsideRange(0x8F),
        Invalid::FirstProfileLinked,
        Invalid::ProfileLinked(0x81),
        Invalid::ProfileNotLinked(1),
        Invalid::AppLinkOutsideRange(16),
        Invalid::AppWithoutFilter,
        Invalid::MomentaryBaseLayer,
        Invalid::ZeroDistance,
        Invalid::ZeroHoldTime,
        Invalid::HoldTimeTooLong(u32::MAX),
        Invalid::ZeroDuration,
        Invalid::CurveNotRising,
        Invalid::PercentageAboveHundred(101),
        Invalid::RateOutsideRange {
            value: 0,
            min: 1,
            max: 255,
        },
        Invalid::LightbarRangeOutsideLeds {
            first: 9,
            last: 3,
            leds: 10,
        },
        Invalid::LightbarColorsExceedLeds { count: 16, leds: 8 },
        Invalid::LightbarHostSignalsExceedLimit,
        Invalid::KeyRectInverted,
        Invalid::ConditionNeverHolds,
        Invalid::ZeroPeriod,
        Invalid::MeterMaximumNotPositive,
        Invalid::ZeroSegments,
        Invalid::NoLightbar,
        Invalid::NoHostSignals,
        Invalid::SwitchModelUnknown(64),
        Invalid::TravelOffsetReachesTravel {
            model: 48,
            offset: 80,
            travel: 80,
        },
        Invalid::TunedSwitchesExceedLimit,
        Invalid::LayoutMissing(Layout::Jis),
        Invalid::MacroOutsideRange(u8::MAX),
        Invalid::MacroInMacro,
        Invalid::ActionNotPlayable,
        Invalid::MacroTooLong(u16::MAX),
        Invalid::CharacterNotTypable(0x10_FFFF),
    ];
    let others = [
        Rejection::Unsupported,
        Rejection::ProfileMissing(3),
        Rejection::ProfileLimitReached,
        Rejection::ProfileProtected(0),
        Rejection::AppLinkMissing(15),
        Rejection::Locked,
        Rejection::StorageInaccessible,
    ];
    for rejection in invalid.map(Rejection::Invalid).into_iter().chain(others) {
        agrees_with_postcard(&Err::<(), _>(rejection));
        agrees_with_postcard(&Err::<keys::Settings, _>(rejection));
    }
}

#[test]
fn every_action_and_advanced_key_encodes_as_postcard_does() {
    let actions = [
        Action::None,
        Action::Keyboard(0x04),
        Action::Consumer(0x02A2),
        Action::System(SystemControl::WakeUp),
        Action::Mouse(MouseButton::Forward),
        Action::Gamepad(Control::HatRight),
        Action::Layer(Layer::Three),
        Action::Function(Function::BrightnessUp),
        Action::Function(Function::BrightnessDown),
        Action::Function(Function::Profile(Slot::new(3))),
        Action::Function(Function::NextProfile),
        Action::Function(Function::ToggleModeProfile),
        Action::Function(Function::ToggleKeyDisable),
        Action::Function(Function::LayerLock),
        Action::Macro(31),
        widest_action(),
    ];
    for action in actions {
        agrees_with_postcard(&action);
    }
    let behaviours = [
        widest_advanced_key().behaviour,
        Behaviour::ModTap(ModTap {
            tap: Action::Keyboard(0x29),
            hold: Action::Keyboard(0xE0),
            hold_time_ms: u16::MAX,
            interrupt: Interrupt::Tap,
        }),
        Behaviour::Toggle(Action::Mouse(MouseButton::Left)),
        Behaviour::DepthPriority(DepthPriority {
            other: Index::new(9),
            both_bottomed_out: Resolution::LastPressed,
        }),
        Behaviour::Socd(Socd {
            other: Index::new(127),
            resolution: Resolution::Second,
            both_when_bottomed_out: true,
        }),
    ];
    let keys: advanced::Keys = behaviours
        .into_iter()
        .map(|behaviour| advanced::Key {
            key: Index::new(1),
            layer: Layer::One,
            behaviour,
        })
        .collect();
    agrees_with_postcard(&keys);
    for override_ in [
        Override::default(),
        widest_override(),
        Override {
            actuation: None,
            rapid_trigger: RapidTriggerOverride::On,
        },
        Override {
            actuation: Some(Distance::from_fraction(0x80)),
            rapid_trigger: RapidTriggerOverride::Off,
        },
    ] {
        agrees_with_postcard(&override_);
    }
    agrees_with_postcard(&key_settings());
    agrees_with_postcard(&RapidTrigger {
        press: Distance::from_fraction(0x3FFF),
        release: Some(Distance::from_fraction(0x4000)),
        continuous: true,
    });
}

#[test]
fn every_light_bar_part_encodes_as_postcard_does() {
    let corner = keys::Position { row: 0, column: 0 };
    let effects = [
        Effect::Mirror(Mirror {
            range: Range { first: 0, last: 7 },
            source: KeyRect {
                first: corner,
                last: keys::Position { row: 5, column: 16 },
            },
        }),
        Effect::Blink(Blink {
            from: opaque(1, 2, 3),
            to: lighting::Color::TRANSPARENT,
            period_ms: 60_000,
            waveform: Waveform::Triangle,
            target: Target::Keys(KeyRect {
                first: corner,
                last: corner,
            }),
        }),
        Effect::Solid(Solid {
            color: opaque(9, 9, 9),
            target: Target::Lightbar(Range { first: 1, last: 2 }),
        }),
    ];
    for effect in effects {
        agrees_with_postcard(&effect);
    }
    for background in [
        Background::ProfileColor,
        Background::Transparent,
        Background::Fixed(opaque(4, 5, 6)),
        Background::Contrast,
        Background::DimmedProfileColor,
    ] {
        agrees_with_postcard(&background);
    }
    let terms = [
        Term::Trigger {
            signal: Signal::Builtin(Builtin::ProfileColor),
            duration_ms: 1,
        },
        Term::Compare(Comparison {
            signal: Signal::Host(0x8000_0000),
            comparator: Comparator::NotEqual,
            operand: Operand::Constant(-0.5),
            hold_ms: None,
        }),
        Term::Compare(Comparison {
            signal: Signal::Builtin(Builtin::KeyRate),
            comparator: Comparator::Greater,
            operand: Operand::Constant(f32::INFINITY),
            hold_ms: Some(0),
        }),
    ];
    for term in terms {
        agrees_with_postcard(&term);
    }
    agrees_with_postcard(&widest_lightbar_entry());
    agrees_with_postcard(&lightbar_settings());
    agrees_with_postcard(&lightbar_settings().entries);
}

#[test]
fn every_unit_variant_encodes_as_postcard_does() {
    every::<Layer>(&Layer::ALL);
    every::<Player>(&Player::ALL);
    every::<BusError>(&diagnostics::BusErrors::KINDS);
    every(&[Mode::Xinput, Mode::Gamepad, Mode::NoGamepad]);
    every(&[KeyboardFormat::NKeyRollover, KeyboardFormat::Boot]);
    every(&[Speed::Full, Speed::High]);
    every(&[
        PollingRate::Every4Milliseconds,
        PollingRate::Every1Millisecond,
        PollingRate::Every125Microseconds,
    ]);
    every(&[Layout::Ansi, Layout::Iso, Layout::Jis]);
    every(&[Shape::Rectangle, Shape::IsoEnter]);
    every(&[Namespace::Onboard, Namespace::Linked]);
    every(&[Matching::Any, Matching::All]);
    every(&[
        Reading::Travel,
        Reading::Value,
        Reading::RestLevel,
        Reading::SingleRestLevel,
        Reading::ParallelRestLevel,
        Reading::BottomLevel,
        Reading::AboveRest,
        Reading::ScanOrder,
    ]);
    every(&[
        HueRange::FullCircle,
        HueRange::From110To184,
        HueRange::From2To39,
        HueRange::From64To114,
        HueRange::From214To254,
        HueRange::From113To204,
    ]);
    every(&[
        Control::ButtonA,
        Control::Guide,
        Control::LeftTrigger,
        Control::Rudder,
        Control::HatRight,
    ]);
    every(&[
        ReactiveColoring::KeyColor,
        ReactiveColoring::Palette,
        ReactiveColoring::Fixed(white().opaque()),
    ]);
}

#[test]
fn app_links_and_profile_parts_encode_as_postcard_does() {
    let mut name = Text::new();
    while name.push('é').is_ok() {}
    let filters: Vec<Filter, { apps::MAX_FILTERS }> = [
        Filter::ProcessName(Text::try_from("game.exe").unwrap()),
        Filter::ProcessDirectory(Text::new()),
        Filter::WindowTitle(name.clone()),
        Filter::ProcessPath(Text::try_from("C:\\🎮").unwrap()),
    ]
    .into_iter()
    .collect();
    let links = [
        Link {
            name: name.clone(),
            app: App::Custom(apps::Custom {
                matching: Matching::All,
                filters,
            }),
        },
        Link {
            name: Text::new(),
            app: App::Steam(u32::MAX),
        },
    ];
    for link in links {
        agrees_with_postcard(&link);
    }
    agrees_with_postcard(&profile::Info {
        name: String::try_from("Profile 1").unwrap(),
        color: white(),
        icon: 255,
    });
    agrees_with_postcard(&gamepad_settings());
    agrees_with_postcard(&lighting_settings());
    agrees_with_postcard(&switches());
    agrees_with_postcard(&switches::Settings {
        default: None,
        keys: Vec::new(),
    });
    agrees_with_postcard(&gamepad::Mapping {
        bindings: [None, Some(Control::ButtonB), Some(Control::HatUp)]
            .into_iter()
            .collect(),
        exclusive: true,
    });
    let geometry: geometry::Keys = [
        None,
        Some(geometry::Key {
            x: u16::MAX,
            y: 0,
            width: 150,
            height: 200,
            shape: Shape::IsoEnter,
        }),
    ]
    .into_iter()
    .collect();
    agrees_with_postcard(&Ok::<_, Rejection>(geometry));
    agrees_with_postcard(&device::Name::try_from("Wooting 60HE+").unwrap());
    agrees_with_postcard(&lightbar::HostSignal {
        id: 7,
        value: f32::MIN_POSITIVE,
    });
}

fn taken_like_postcard(bytes: &[u8]) {
    let taken = Encoded::take(bytes).map(|(value, rest)| {
        (
            postcard::from_bytes::<Macro>(value.as_bytes()).ok(),
            rest.len(),
        )
    });
    let expected = postcard::take_from_bytes::<Macro>(bytes)
        .ok()
        .map(|(value, rest)| (Some(value), rest.len()));
    assert_eq!(taken, expected, "{bytes:02x?}");
}

#[test]
fn macros_encode_as_postcard_does_and_their_wire_form_ends_where_postcard_ends_them() {
    for build in [sample_macro, largest_macro, Macro::default] {
        let value = build();
        agrees_with_postcard(&value);
        agrees_with_postcard(&Summary::from(&value));
        agrees_with_postcard(&macros::Change {
            index: 31,
            value: value.clone(),
        });
        let encoded = wire_bytes(&value);
        for bytes in mutations(&encoded) {
            taken_like_postcard(&bytes);
        }
        let summary = Encoded::parse(&encoded).map(|value| value.summary());
        assert_eq!(summary, Some(Summary::from(&value)));
    }
    let summaries: macros::Summaries = [Some(Summary::from(&sample_macro())), None]
        .into_iter()
        .collect();
    agrees_with_postcard(&summaries);
}

#[test]
fn random_bytes_decode_as_postcard_decodes_them() {
    let mut seed = 0x0B5E_55ED_u32;
    for length in (0..48).chain([64, 100, 300, 700, 1500]) {
        for _ in 0..64 {
            let bytes: std::vec::Vec<u8> = (0..length)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    seed.to_le_bytes()[3]
                })
                .collect();
            decodes_like_postcard::<Action>(&bytes);
            decodes_like_postcard::<keys::Settings>(&bytes);
            decodes_like_postcard::<keys::Keymap>(&bytes);
            decodes_like_postcard::<keys::Overrides>(&bytes);
            decodes_like_postcard::<advanced::Keys>(&bytes);
            decodes_like_postcard::<gamepad::Mapping>(&bytes);
            decodes_like_postcard::<gamepad::Settings>(&bytes);
            decodes_like_postcard::<lighting::Settings>(&bytes);
            decodes_like_postcard::<Option<lighting::KeyColors>>(&bytes);
            decodes_like_postcard::<lightbar::Settings>(&bytes);
            decodes_like_postcard::<lightbar::HostSignals>(&bytes);
            decodes_like_postcard::<switches::Settings>(&bytes);
            decodes_like_postcard::<profile::Info>(&bytes);
            decodes_like_postcard::<apps::LinkChange>(&bytes);
            decodes_like_postcard::<device::Settings>(&bytes);
            decodes_like_postcard::<device::Name>(&bytes);
            decodes_like_postcard::<profile::Selection>(&bytes);
            decodes_like_postcard::<Result<u32, Rejection>>(&bytes);
            decodes_like_postcard::<usize>(&bytes);
            decodes_like_postcard::<Macro>(&bytes);
            decodes_like_postcard::<macros::Change>(&bytes);
            taken_like_postcard(&bytes);
        }
    }
}

fn every<T>(values: &[T])
where
    T: Encode + Decode + Serialize + DeserializeOwned + Debug,
{
    for value in values {
        agrees_with_postcard(value);
    }
}
