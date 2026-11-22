use openwoot_protocol::{
    advanced::{self, Behaviour, DynamicKeystroke, Stroke, StrokeEvent},
    config::{Capabilities, Rejection},
    device::Layout,
    gamepad::{self, CurvePoint, Normalization},
    heapless::Vec,
    keys::{
        self, Action, Distance, Index, Layer, Override, RapidTrigger, RapidTriggerOverride,
        ScanMode, TapDuration,
    },
    lightbar::{
        self, Background, Builtin, Comparator, Comparison, Condition, Custom, Entry, Group, Logic,
        Meter, Operand, ProfileIndicator, Range, Signal, Term,
    },
    lighting::{
        self, AnalogLevel, Color, Effect, Indicators, ReactiveColoring, ReactiveFade, Rgb, Scanner,
        ScannerDirection,
    },
    switches::{self, Switch},
};

pub(crate) const DONE: Result<(), Rejection> = Ok(());
pub(crate) const SEQUENCE: u16 = 0x0102;

macro_rules! round_trip {
    ($endpoint:ty, $request:expr, $response:expr $(,)?) => {{
        use openwoot_protocol::{
            Endpoint, decode_message, decode_response, encode_message, encode_request, report,
        };

        use crate::support::SEQUENCE;

        let request = $request;
        let response = $response;
        let mut report = [0; report::MAX_LEN];
        let encoded = encode_request::<$endpoint>(SEQUENCE, &request, &mut report).unwrap();
        let message = decode_message(encoded).unwrap().unwrap();
        assert_eq!(
            postcard::from_bytes::<<$endpoint as Endpoint>::Request>(message.body).unwrap(),
            request
        );
        let mut reply = [0; report::MAX_LEN];
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

pub(crate) use round_trip;

pub(crate) fn capabilities() -> Capabilities {
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

pub(crate) fn key_count() -> usize {
    capabilities().key_count()
}

pub(crate) fn filled<T: Clone, const N: usize>(value: T, len: usize) -> Vec<T, N> {
    core::iter::repeat_n(value, len).collect()
}

pub(crate) fn widest_action() -> Action {
    Action::Consumer(0x02A2)
}

pub(crate) fn widest_override() -> Override {
    Override {
        actuation: Some(Distance::FULL_TRAVEL),
        rapid_trigger: RapidTriggerOverride::With(RapidTrigger {
            press: Distance::FULL_TRAVEL,
            release: Some(Distance::FULL_TRAVEL),
            continuous: true,
        }),
    }
}

pub(crate) fn widest_advanced_key() -> advanced::Key {
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

pub(crate) fn widest_lightbar_entry() -> Entry {
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

pub(crate) fn white() -> Rgb {
    Rgb {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    }
}

pub(crate) fn opaque(red: u8, green: u8, blue: u8) -> Color {
    Rgb { red, green, blue }.opaque()
}

pub(crate) fn lighting_settings() -> lighting::Settings {
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

pub(crate) fn lightbar_settings() -> lightbar::Settings {
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

pub(crate) fn gamepad_settings() -> gamepad::Settings {
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

pub(crate) fn key_settings() -> keys::Settings {
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

pub(crate) fn switches() -> switches::Settings {
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
