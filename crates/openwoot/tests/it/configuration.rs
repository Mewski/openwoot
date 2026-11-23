use core::future::{Future, ready};

use embassy_futures::block_on;
use openwoot::{
    protocol::{Configuration, Handler},
    usb::HostState,
};
use openwoot_protocol::{
    Endpoint, FirmwareVersion, PROTOCOL_VERSION, VarHeader, VarKey, VarSeq, Version, WireError,
    configuration::{Capabilities, Invalid, Rejection},
    decode_response, encode_message, encode_request,
    endpoints::{
        ActivateProfile, GetCapabilities, GetKeymap, GetVersion, Save, SetGamepadDetection,
        SetKeyReports, SetKeymap, SetLighting,
    },
    heapless::Vec,
    keys::{Action, Keymap, Layer},
    lighting::{self, Indicators},
    profile::{Change, LayerChange, LayerRef, Slot},
    report,
};

use crate::support::stock_identity;

const SEQUENCE: u16 = 0x0BAD;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 4,
        patch: 2,
    },
};

#[derive(Debug, Default)]
struct Memory {
    keymap: Option<(Slot, Layer, Keymap)>,
    changes: usize,
    saves: usize,
}

impl Configuration for Memory {
    fn capabilities(&self) -> Result<Capabilities, Rejection> {
        Ok(Capabilities {
            key_rows: 2,
            key_columns: 3,
            profiles: 4,
            layouts: Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 1,
            edition: None,
        })
    }

    fn activate_profile(&mut self, profile: Slot) -> impl Future<Output = Result<(), Rejection>> {
        ready(if profile.get() < 2 {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        })
    }

    fn keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &mut Keymap,
    ) -> impl Future<Output = Result<(), Rejection>> {
        ready(match &self.keymap {
            Some((stored_profile, stored_layer, stored))
                if (*stored_profile, *stored_layer) == (profile, layer) =>
            {
                keymap.clone_from(stored);
                Ok(())
            }
            _ => Err(Rejection::ProfileMissing(profile.get())),
        })
    }

    fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &Keymap,
    ) -> impl Future<Output = Result<(), Rejection>> {
        self.changes += 1;
        self.keymap = Some((profile, layer, keymap.clone()));
        ready(Ok(()))
    }

    fn save(&mut self) -> impl Future<Output = Result<(), Rejection>> {
        self.saves += 1;
        ready(Ok(()))
    }
}

macro_rules! ask {
    ($handler:expr, $endpoint:ty, $request:expr) => {{
        let mut request = [0; report::MAX_LEN];
        let request = encode_request::<$endpoint>(SEQUENCE, &$request, &mut request).unwrap();
        let mut response = [0; report::MAX_LEN];
        let answer = block_on($handler.respond(request, &mut response));
        assert_eq!(answer.handover, None);
        decode_response::<$endpoint>(SEQUENCE, answer.reply.unwrap())
    }};
}

fn configured() -> Handler<Memory> {
    Handler::with_configuration(VERSION, stock_identity(None), Memory::default())
}

fn keymap(action: Action) -> Keymap {
    core::iter::once(action)
        .chain(core::iter::repeat_n(Action::None, 5))
        .collect()
}

fn layer_change(profile: u8, action: Action) -> LayerChange<Keymap> {
    LayerChange {
        profile: Slot::new(profile),
        layer: Layer::Two,
        value: keymap(action),
    }
}

#[test]
fn a_keyboard_without_configuration_still_answers_its_version_and_refuses_the_rest() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    assert_eq!(ask!(handler, GetVersion, ()), Ok(Some(VERSION)));
    assert_eq!(
        ask!(handler, GetCapabilities, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, SetKeymap, layer_change(0, Action::Keyboard(0x04))),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, Save, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
}

#[test]
fn a_valid_keymap_reaches_the_configuration_and_reads_back() {
    let mut handler = configured();
    assert_eq!(
        ask!(handler, SetKeymap, layer_change(1, Action::Keyboard(0x2C))),
        Ok(Some(Ok(())))
    );
    assert_eq!(
        ask!(
            handler,
            GetKeymap,
            LayerRef {
                profile: Slot::new(1),
                layer: Layer::Two,
            }
        ),
        Ok(Some(Ok(keymap(Action::Keyboard(0x2C)))))
    );
    assert_eq!(ask!(handler, Save, ()), Ok(Some(Ok(()))));
    assert_eq!(handler.configuration().saves, 1);
}

#[test]
fn invalid_values_are_rejected_before_the_configuration_sees_them() {
    let mut handler = configured();
    assert_eq!(
        ask!(handler, SetKeymap, layer_change(1, Action::Keyboard(0xE9))),
        Ok(Some(Err(Rejection::Invalid(
            Invalid::KeyboardUsageOutsideReport(0xE9)
        ))))
    );
    assert_eq!(
        ask!(handler, SetKeymap, layer_change(4, Action::None)),
        Ok(Some(Err(Rejection::Invalid(Invalid::ProfileOutsideRange(
            4
        )))))
    );
    let short = LayerChange {
        value: keymap(Action::None).into_iter().take(5).collect(),
        ..layer_change(0, Action::None)
    };
    assert_eq!(
        ask!(handler, SetKeymap, short),
        Ok(Some(Err(Rejection::Invalid(
            Invalid::KeyCountDiffersFromMatrix {
                expected: 6,
                found: 5
            }
        ))))
    );
    assert_eq!(handler.configuration().changes, 0);
}

#[test]
fn rejections_of_the_configuration_reach_the_host() {
    let mut handler = configured();
    assert_eq!(
        ask!(handler, ActivateProfile, Slot::new(1)),
        Ok(Some(Ok(())))
    );
    assert_eq!(
        ask!(handler, ActivateProfile, Slot::new(3)),
        Ok(Some(Err(Rejection::ProfileMissing(3))))
    );
}

#[test]
fn a_part_the_configuration_does_not_implement_is_unsupported() {
    let mut handler = configured();
    let lighting = Change {
        profile: Slot::new(0),
        value: lighting::Settings {
            brightness: 255,
            indicators: Indicators {
                caps_lock: lighting::Color::TRANSPARENT,
                scroll_lock: lighting::Color::TRANSPARENT,
                num_lock: lighting::Color::TRANSPARENT,
                layer_lock: lighting::Color::TRANSPARENT,
            },
            effects: Vec::new(),
        },
    };
    assert_eq!(
        ask!(handler, SetLighting, lighting),
        Ok(Some(Err(Rejection::Unsupported)))
    );
}

#[test]
fn a_body_that_does_not_decode_answers_deser_failed() {
    let mut handler = configured();
    let header = VarHeader {
        key: VarKey::Key8(SetKeymap::REQ_KEY),
        seq_no: VarSeq::Seq2(SEQUENCE),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &(), &mut request).unwrap();
    let mut response = [0; report::MAX_LEN];
    let answer = block_on(handler.respond(request, &mut response));
    assert_eq!(
        decode_response::<SetKeymap>(SEQUENCE, answer.reply.unwrap()),
        Err(openwoot_protocol::Error::Device(WireError::DeserFailed))
    );
    assert_eq!(handler.configuration().changes, 0);
}

#[test]
fn key_reports_and_gamepad_detection_switch_the_host_state() {
    static HOST: HostState = HostState::new();
    let mut handler = configured().with_host(&HOST);
    assert_eq!(ask!(handler, SetKeyReports, false), Ok(Some(Ok(()))));
    assert!(!HOST.key_reports());
    assert_eq!(ask!(handler, SetGamepadDetection, true), Ok(Some(Ok(()))));
    assert!(HOST.gamepad_detection());
    assert_eq!(ask!(handler, SetKeyReports, true), Ok(Some(Ok(()))));
    assert!(HOST.key_reports());
}

#[test]
fn key_reports_and_gamepad_detection_are_unsupported_without_host_state() {
    let mut handler = configured();
    assert_eq!(
        ask!(handler, SetKeyReports, false),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, SetGamepadDetection, true),
        Ok(Some(Err(Rejection::Unsupported)))
    );
}

#[test]
fn a_getter_for_a_profile_beyond_the_capabilities_is_rejected_before_the_configuration_sees_it() {
    let mut handler = configured();
    for profile in [4, u8::MAX] {
        assert_eq!(
            ask!(
                handler,
                GetKeymap,
                LayerRef {
                    profile: Slot::new(profile),
                    layer: Layer::Base,
                }
            ),
            Ok(Some(Err(Rejection::Invalid(Invalid::ProfileOutsideRange(
                profile
            )))))
        );
    }
}
