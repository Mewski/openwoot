use core::fmt::Debug;

use openwoot_protocol::{
    advanced::{self, Behaviour, DynamicKeystroke, Stroke, StrokeEvent},
    configuration::{Capabilities, Rejection},
    device::Layout,
    gamepad::{self, CurvePoint, Normalization, StickBias},
    heapless::Vec,
    keys::{
        self, Action, Distance, Index, Layer, Override, RapidTrigger, RapidTriggerOverride,
        ScanMode, TapDuration,
    },
    lightbar::{
        self, Background, Builtin, Comparator, Comparison, Condition, Custom, Entry, Group, Logic,
        Meter, Operand, ProfileIndicator, Range, Signal, Term, Title,
    },
    lighting::{
        self, AnalogLevel, Color, Effect, Indicators, ReactiveColoring, ReactiveFade, Rgb, Scanner,
        ScannerDirection,
    },
    switches::{self, Switch},
    wire::{self, Decode, Encode},
};
use serde::{Serialize, de::DeserializeOwned};

pub(crate) const DONE: Result<(), Rejection> = Ok(());
pub(crate) const SEQUENCE: u16 = 0x0102;
const LARGEST_VALUE: usize = 4096;

macro_rules! round_trip {
    ($endpoint:ty, $request:expr, $response:expr $(,)?) => {{
        use openwoot_protocol::{
            Endpoint, decode_message, decode_response, encode_frame, encode_message,
            encode_request, report,
        };

        use crate::support::{SEQUENCE, agrees_with_postcard};

        let request = $request;
        let response = $response;
        agrees_with_postcard(&request);
        agrees_with_postcard(&response);
        let mut report = [0; report::MAX_LEN];
        let encoded = encode_request::<$endpoint>(SEQUENCE, &request, &mut report).unwrap();
        let message = decode_message(encoded).unwrap().unwrap();
        assert_eq!(
            postcard::from_bytes::<<$endpoint as Endpoint>::Request>(message.body).unwrap(),
            request
        );
        let header = message.reply_header(<$endpoint as Endpoint>::RESP_KEY);
        let mut reply = [0; report::MAX_LEN];
        let len = encode_message(&header, &response, &mut reply)
            .unwrap()
            .len();
        let mut wired = [0; report::MAX_LEN];
        assert_eq!(
            encode_frame(&header, &response, &mut wired),
            Ok(&reply[..len])
        );
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
        linked_profiles: 10,
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
    let mut title = Title::new();
    while title.push('W').is_ok() {}
    Entry {
        title,
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
        title: Title::try_from("Profiles").unwrap_or_default(),
        condition: None,
        effect: lightbar::Effect::ProfileIndicator(ProfileIndicator {
            range: Range { first: 0, last: 9 },
            background: Background::Fixed(opaque(10, 10, 10)),
            segments: 5,
        }),
    };
    let meter = Entry {
        title: Title::new(),
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
        left_stick_up_bias: StickBias {
            enabled: true,
            percent: 50,
        },
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

pub(crate) fn agrees_with_postcard<T>(value: &T)
where
    T: Encode + Decode + Serialize + DeserializeOwned + Debug,
{
    let expected = postcard_bytes(value);
    assert_eq!(wire_bytes(value), expected, "{value:?}");
    for bytes in mutations(&expected) {
        decodes_like_postcard::<T>(&bytes);
    }
}

pub(crate) fn decodes_like_postcard<T>(bytes: &[u8])
where
    T: Encode + Decode + Serialize + DeserializeOwned + Debug,
{
    let wire = wire::take::<T>(bytes).map(|(decoded, rest)| (wire_bytes(&decoded), rest.len()));
    let postcard = postcard::take_from_bytes::<T>(bytes)
        .ok()
        .map(|(decoded, rest)| (postcard_bytes(&decoded), rest.len()));
    assert_eq!(wire, postcard, "{bytes:02x?}");
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

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "every value a test encodes fits the largest value"
)]
fn wire_bytes(value: &impl Encode) -> std::vec::Vec<u8> {
    let mut buffer = [0; LARGEST_VALUE];
    let len = wire::encode(value, &mut buffer).unwrap();
    buffer[..len].to_vec()
}

#[expect(
    clippy::unwrap_used,
    reason = "every value a test encodes fits the largest value"
)]
fn postcard_bytes(value: &impl Serialize) -> std::vec::Vec<u8> {
    let mut buffer = [0; LARGEST_VALUE];
    postcard::to_slice(value, &mut buffer).unwrap().to_vec()
}

#[expect(
    clippy::indexing_slicing,
    reason = "every position and cut lies within the encoded bytes"
)]
fn mutations(bytes: &[u8]) -> impl Iterator<Item = std::vec::Vec<u8>> {
    let edits = (0..bytes.len()).flat_map(move |position| {
        [1_u8, 0x7F, 0x80, 0xFF].into_iter().map(move |delta| {
            let mut mutated = bytes.to_vec();
            mutated[position] = mutated[position].wrapping_add(delta);
            mutated
        })
    });
    let cuts = (0..bytes.len())
        .filter(|len| *len < 64 || len % 16 == 0)
        .map(move |len| bytes[..len].to_vec());
    let extended = [bytes, &[0, 1]].concat();
    core::iter::once(bytes.to_vec())
        .chain(edits)
        .chain(cuts)
        .chain(core::iter::once(extended))
}
