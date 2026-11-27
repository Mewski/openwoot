use core::cell::RefCell;

use embassy_futures::block_on;
use openwoot::{
    keys,
    macros::{Header, Loaded},
    profile::{self, Configurator},
    protocol::{Configuration, Handler},
};
use openwoot_protocol::{
    FirmwareVersion, PROTOCOL_VERSION, Version, WireError,
    configuration::{Invalid, Rejection},
    decode_response,
    device::Layout,
    encode_request,
    endpoints::{ClearMacro, FactoryReset, GetMacro, GetMacros, Revert, SetMacro},
    keys::{Action, Function, MouseButton},
    macros::{
        Change, Encoded, MAX_ENCODED_LEN, MAX_MACROS, Macro, Mode, Name, Step, Summaries, Summary,
        Text,
    },
    report, wire,
};

use crate::{
    configuration::at_once,
    profile::{open, storage, switch_capabilities},
    support::stock_identity,
};

const SEQUENCE: u16 = 0x0D0D;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 4,
        patch: 2,
    },
};

macro_rules! ask {
    ($handler:expr, $endpoint:ty, $request:expr) => {{
        let mut request = [0; report::MAX_LEN];
        let request = encode_request::<$endpoint>(SEQUENCE, &$request, &mut request).unwrap();
        let mut response = [0; report::MAX_LEN];
        let answer = block_on($handler.respond(request, &mut response));
        assert_eq!(answer.handover, None);
        decode_response::<$endpoint>(SEQUENCE, answer.reply.unwrap())
            .unwrap()
            .unwrap()
    }};
}

#[expect(
    clippy::unwrap_used,
    reason = "the name and text of the example fit their capacities"
)]
fn greeting() -> Macro {
    Macro {
        name: Name::try_from("Greeting").unwrap(),
        mode: Mode::WhileHeld,
        steps: [
            Step::Press(Action::Keyboard(0xE1)),
            Step::Tap(Action::Keyboard(0x0B)),
            Step::Release(Action::Keyboard(0xE1)),
            Step::Delay(250),
            Step::Text(Text::try_from("ello, world!\n").unwrap()),
            Step::MouseMove { x: -400, y: 12 },
            Step::Scroll {
                vertical: 2,
                horizontal: -1,
            },
            Step::Tap(Action::Mouse(MouseButton::Middle)),
        ]
        .into_iter()
        .collect(),
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "the largest macro is built step by step up to its byte limit"
)]
fn largest() -> Macro {
    let mut name = Name::new();
    while name.push('W').is_ok() {}
    let mut text = Text::new();
    while text.push('~').is_ok() {}
    let mut value = Macro {
        name,
        mode: Mode::Toggle,
        steps: [].into_iter().collect(),
    };
    while value.encoded_len() + wire::encoded_len(&Step::Text(text.clone())) <= MAX_ENCODED_LEN {
        value.steps.push(Step::Text(text.clone())).unwrap();
    }
    while value.encoded_len() < MAX_ENCODED_LEN {
        let left = MAX_ENCODED_LEN - value.encoded_len();
        value
            .steps
            .push(Step::Delay(if left == 3 { 0x80 } else { 0 }))
            .unwrap();
    }
    value
}

fn empty_library() -> Summaries {
    core::iter::repeat_n(None, usize::from(MAX_MACROS)).collect()
}

#[test]
fn a_macro_is_stored_at_once_read_back_listed_and_cleared() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    assert_eq!(ask!(handler, GetMacros, ()), Ok(empty_library()));
    assert_eq!(ask!(handler, GetMacro, 3), Ok(None));
    let change = Change {
        index: 3,
        value: greeting(),
    };
    assert_eq!(ask!(handler, SetMacro, change), Ok(()));
    assert_eq!(ask!(handler, GetMacro, 3), Ok(Some(greeting())));
    let mut listed = empty_library();
    listed[3] = Some(Summary {
        name: greeting().name,
        mode: Mode::WhileHeld,
        steps: 8,
    });
    assert_eq!(ask!(handler, GetMacros, ()), Ok(listed));
    assert_eq!(ask!(handler, ClearMacro, 3), Ok(()));
    assert_eq!(ask!(handler, GetMacro, 3), Ok(None));
    assert_eq!(ask!(handler, ClearMacro, 3), Ok(()));
    assert_eq!(ask!(handler, GetMacros, ()), Ok(empty_library()));
}

#[test]
fn storing_the_same_macro_again_writes_nothing() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let change = Change {
        index: 0,
        value: greeting(),
    };
    assert_eq!(ask!(handler, SetMacro, change), Ok(()));
    let used = handler.configuration().storage_usage().unwrap();
    assert_eq!(ask!(handler, SetMacro, change), Ok(()));
    assert_eq!(handler.configuration().storage_usage(), Ok(used));
}

#[test]
fn macros_survive_a_revert_and_go_with_a_factory_reset() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    {
        let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
        let mut handler = Handler::with_configuration(
            VERSION,
            stock_identity(None),
            Configurator::new(&model, switch_capabilities()),
        );
        let change = Change {
            index: MAX_MACROS - 1,
            value: greeting(),
        };
        assert_eq!(ask!(handler, SetMacro, change), Ok(()));
        assert_eq!(ask!(handler, Revert, ()), Ok(()));
        assert_eq!(
            ask!(handler, GetMacro, MAX_MACROS - 1),
            Ok(Some(greeting()))
        );
    }
    let reopened = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&reopened, switch_capabilities()),
    );
    assert_eq!(
        ask!(handler, GetMacro, MAX_MACROS - 1),
        Ok(Some(greeting()))
    );
    assert_eq!(ask!(handler, FactoryReset, ()), Ok(()));
    assert_eq!(ask!(handler, GetMacro, MAX_MACROS - 1), Ok(None));
    assert_eq!(ask!(handler, GetMacros, ()), Ok(empty_library()));
}

#[test]
fn the_largest_macro_goes_through_one_frame_each_way() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let value = largest();
    assert_eq!(value.encoded_len(), MAX_ENCODED_LEN);
    let change = Change {
        index: 7,
        value: value.clone(),
    };
    assert_eq!(ask!(handler, SetMacro, change), Ok(()));
    assert_eq!(ask!(handler, GetMacro, 7), Ok(Some(value)));
}

#[test]
fn macros_the_keyboard_cannot_play_are_refused_before_they_reach_the_store() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let with_step = |step: Step| Macro {
        steps: [step].into_iter().collect(),
        ..greeting()
    };
    let mut too_long = largest();
    too_long.steps.push(Step::Delay(0)).unwrap();
    let mut refuses = |index: u8, value: Macro, invalid: Invalid| {
        assert_eq!(
            ask!(handler, SetMacro, Change { index, value }),
            Err(Rejection::Invalid(invalid))
        );
    };
    refuses(
        MAX_MACROS,
        greeting(),
        Invalid::MacroOutsideRange(MAX_MACROS),
    );
    refuses(
        0,
        with_step(Step::Tap(Action::Macro(1))),
        Invalid::MacroInMacro,
    );
    refuses(
        0,
        with_step(Step::Press(Action::Function(Function::NextProfile))),
        Invalid::ActionNotPlayable,
    );
    refuses(
        0,
        with_step(Step::Text(Text::try_from("café").unwrap())),
        Invalid::CharacterNotTypable(0xE9),
    );
    refuses(
        0,
        with_step(Step::Release(Action::Consumer(0x02A3))),
        Invalid::ConsumerUsageOutsideRange(0x02A3),
    );
    refuses(0, too_long, Invalid::MacroTooLong(2002));
    assert_eq!(ask!(handler, GetMacros, ()), Ok(empty_library()));
    assert_eq!(
        ask!(handler, GetMacro, 40),
        Err(Rejection::Invalid(Invalid::MacroOutsideRange(40)))
    );
    assert_eq!(
        ask!(handler, ClearMacro, MAX_MACROS),
        Err(Rejection::Invalid(Invalid::MacroOutsideRange(MAX_MACROS)))
    );
}

#[test]
fn a_set_macro_body_that_does_not_decode_or_runs_on_answers_deser_failed() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let mut buffer = [0; report::MAX_LEN];
    let len = wire::encode(
        &Change {
            index: 2,
            value: greeting(),
        },
        &mut buffer,
    )
    .unwrap();
    let body = &buffer[..len];
    let deser_failed = Err(openwoot_protocol::Error::Device(WireError::DeserFailed));
    assert_eq!(
        at_once::<SetMacro, _>(&mut handler, &body[..len - 1]),
        deser_failed
    );
    assert_eq!(
        at_once::<SetMacro, _>(&mut handler, &[body, &[0]].concat()),
        deser_failed
    );
    let mut wrong_step = body.to_vec();
    let first_step = 1 + Encoded::parse(&body[1..]).unwrap().as_bytes().len()
        - Encoded::parse(&body[1..]).unwrap().encoded_steps().len();
    wrong_step[first_step] = 7;
    assert_eq!(
        at_once::<SetMacro, _>(&mut handler, &wrong_step),
        deser_failed
    );
    assert_eq!(
        at_once::<GetMacro, _>(&mut handler, &[]),
        Err(openwoot_protocol::Error::Device(WireError::DeserFailed))
    );
    assert_eq!(ask!(handler, GetMacro, 2), Ok(None));
    assert_eq!(at_once::<SetMacro, _>(&mut handler, body), Ok(Some(Ok(()))));
}

#[test]
fn a_keyboard_without_a_macro_library_answers_unsupported() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    assert_eq!(ask!(handler, GetMacros, ()), Err(Rejection::Unsupported));
    assert_eq!(ask!(handler, GetMacro, 0), Err(Rejection::Unsupported));
    let change = Change {
        index: 0,
        value: greeting(),
    };
    assert_eq!(ask!(handler, SetMacro, change), Err(Rejection::Unsupported));
    assert_eq!(ask!(handler, ClearMacro, 0), Err(Rejection::Unsupported));
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let storeless = RefCell::new(open(&keys, &shared, None));
    let mut storeless = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&storeless, switch_capabilities()),
    );
    assert_eq!(ask!(storeless, GetMacro, 0), Ok(None));
    let change = Change {
        index: 0,
        value: greeting(),
    };
    assert_eq!(
        ask!(storeless, SetMacro, change),
        Err(Rejection::StorageInaccessible)
    );
}

#[test]
fn thread_mode_loads_a_wanted_macro_with_the_layout_in_effect() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = Configurator::new(&model, switch_capabilities());
    let mut buffer = [0; MAX_ENCODED_LEN];
    let len = wire::encode(&greeting(), &mut buffer).unwrap();
    let encoded = Encoded::parse(&buffer[..len]).unwrap();
    assert_eq!(configurator.set_macro(4, encoded), Ok(()));
    let playback = keys.macros();
    playback.want(4);
    model.borrow_mut().load_macro(4, Layout::Iso);
    assert_eq!(
        playback.loaded(4),
        Loaded::Ready(Header {
            mode: Mode::WhileHeld,
            layout: Layout::Iso,
            steps: 8,
        })
    );
    assert_eq!(
        playback.step(4, 0),
        Some((Step::Press(Action::Keyboard(0xE1)), 3))
    );
    playback.want(5);
    model.borrow_mut().load_macro(5, Layout::Iso);
    assert_eq!(playback.loaded(5), Loaded::Missing);
    playback.want(4);
    model.borrow_mut().load_macro(MAX_MACROS, Layout::Ansi);
    assert_eq!(playback.loaded(4), Loaded::Pending);
}
