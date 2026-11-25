use core::{cell::RefCell, fmt::Debug, pin::pin};

use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use openwoot::{
    keys,
    lighting::{Bar, HostControl, Lightbar},
    profile::{self, Configurator, Model},
    protocol::{Configuration, Handler},
    usb::HostState,
};
use openwoot_protocol::{
    Body, Endpoint, FirmwareVersion, PROTOCOL_VERSION, VarHeader, VarKey, VarSeq, Version,
    WireError, advanced,
    configuration::{Capabilities, Invalid, Rejection},
    decode_response,
    device::{Layout, NAME_LEN, Name},
    diagnostics::StorageUsage,
    encode_message, encode_request,
    endpoints::{
        ActivateProfile, GetCapabilities, GetDefaultKeymap, GetDeviceName, GetGamepadState,
        GetGlobalLightbar, GetHostSignals, GetKeyGeometry, GetKeymap, GetLayout, GetLightbar,
        GetLightbarColors, GetProfileInfo, GetProfiles, GetStorageUsage, GetSwitchModels,
        GetSwitches, GetVersion, MoveProfile, ResetHostLightbar, Revert, Save, SetGamepadDetection,
        SetGlobalLightbar, SetHostLightbar, SetHostSignals, SetKeyReports, SetKeymap, SetLayout,
        SetLightbar, SetLighting, SetSwitches, SwapProfiles,
    },
    gamepad, geometry,
    heapless::Vec,
    keys::{Action, Keymap, Layer},
    lightbar,
    lighting::{self, Indicators},
    profile::{Change, LayerChange, LayerRef, Move, Slot, Summary, Swap},
    report, switches,
};
use serde::{Serialize, Serializer, de::DeserializeOwned, ser::SerializeTuple};

use crate::{
    lighting::{COLUMNS, ROWS, START_MS, lit_lighting, read, red_bar},
    profile::{FACTORY, chosen, factory_switches, layouts, open, storage, switch_capabilities},
    storage::{Nor, SECTOR},
    support::{poll, stock_identity},
};

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
            linked_profiles: 0,
            layouts: Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 1,
            edition: None,
        })
    }

    fn activate_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        if profile.get() < 2 {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        }
    }

    fn keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &mut Keymap,
    ) -> Result<(), Rejection> {
        match &self.keymap {
            Some((stored_profile, stored_layer, stored))
                if (*stored_profile, *stored_layer) == (profile, layer) =>
            {
                keymap.clone_from(stored);
                Ok(())
            }
            _ => Err(Rejection::ProfileMissing(profile.get())),
        }
    }

    fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &Keymap,
    ) -> Result<(), Rejection> {
        self.changes += 1;
        self.keymap = Some((profile, layer, keymap.clone()));
        Ok(())
    }

    fn save(&mut self) -> Result<(), Rejection> {
        self.saves += 1;
        Ok(())
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

macro_rules! reply {
    ($handler:expr, $endpoint:ty, $request:expr) => {{
        let mut request = [0; report::MAX_LEN];
        let request = encode_request::<$endpoint>(SEQUENCE, &$request, &mut request).unwrap();
        let mut response = [0; report::MAX_LEN];
        block_on($handler.respond(request, &mut response))
            .reply
            .unwrap()
            .to_vec()
    }};
}

macro_rules! typed {
    ($endpoint:ty, $response:expr) => {{
        let header = VarHeader {
            key: VarKey::Key8(<$endpoint as Endpoint>::RESP_KEY),
            seq_no: VarSeq::Seq2(SEQUENCE),
        };
        let response: <$endpoint as Endpoint>::Response = $response;
        let mut report = [0; report::MAX_LEN];
        encode_message(&header, &response, &mut report)
            .unwrap()
            .to_vec()
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

#[derive(Debug)]
struct LongName;

impl Configuration for LongName {
    fn device_name(&mut self) -> Result<Name, Rejection> {
        Ok(long_name())
    }
}

fn long_name() -> Name {
    core::iter::repeat_n('n', NAME_LEN).collect()
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
    assert_eq!(
        ask!(handler, GetKeyGeometry, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, GetDefaultKeymap, Layer::One),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, GetGamepadState, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(
            handler,
            SwapProfiles,
            Swap {
                first: Slot::new(0),
                second: Slot::new(1),
            }
        ),
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
fn a_reply_that_outgrows_the_response_is_answered_ser_failed_in_its_place() {
    let mut handler = Handler::with_configuration(VERSION, stock_identity(None), LongName);
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetDeviceName>(SEQUENCE, &(), &mut request).unwrap();
    let mut response = [0; 1 + report::SIZES[0]];
    let answer = block_on(handler.respond(request, &mut response));
    assert_eq!(answer.handover, None);
    assert_eq!(
        decode_response::<GetDeviceName>(SEQUENCE, answer.reply.unwrap()),
        Err(openwoot_protocol::Error::Device(WireError::SerFailed))
    );
    assert_eq!(ask!(handler, GetDeviceName, ()), Ok(Some(Ok(long_name()))));
}

#[test]
fn replies_carry_exactly_the_bytes_of_their_typed_encoding() {
    let mut handler = configured();
    let stored = LayerRef {
        profile: Slot::new(1),
        layer: Layer::Two,
    };
    assert_eq!(reply!(handler, GetVersion, ()), typed!(GetVersion, VERSION));
    assert_eq!(
        reply!(handler, SetKeymap, layer_change(1, Action::Keyboard(0x2C))),
        typed!(SetKeymap, Ok(()))
    );
    assert_eq!(
        reply!(handler, GetKeymap, stored),
        typed!(GetKeymap, Ok(keymap(Action::Keyboard(0x2C))))
    );
    assert_eq!(
        reply!(
            handler,
            GetKeymap,
            LayerRef {
                profile: Slot::new(0),
                ..stored
            }
        ),
        typed!(GetKeymap, Err(Rejection::ProfileMissing(0)))
    );
    assert_eq!(
        reply!(handler, ActivateProfile, Slot::new(4)),
        typed!(
            ActivateProfile,
            Err(Rejection::Invalid(Invalid::ProfileOutsideRange(4)))
        )
    );
    let capabilities = handler.configuration().capabilities();
    assert_eq!(
        reply!(handler, GetCapabilities, ()),
        typed!(GetCapabilities, capabilities)
    );
}

#[test]
fn key_reports_and_gamepad_detection_switch_the_host_state_and_the_gamepad_state_reports_it() {
    static HOST: HostState = HostState::new();
    let mut handler = configured().with_host(&HOST);
    assert_eq!(
        ask!(handler, GetGamepadState, ()),
        Ok(Some(Ok(gamepad::State {
            player: None,
            detection: false,
        })))
    );
    assert_eq!(ask!(handler, SetKeyReports, false), Ok(Some(Ok(()))));
    assert!(!HOST.key_reports());
    assert_eq!(ask!(handler, SetGamepadDetection, true), Ok(Some(Ok(()))));
    assert!(HOST.gamepad_detection());
    assert_eq!(
        ask!(handler, GetGamepadState, ()),
        Ok(Some(Ok(gamepad::State {
            player: None,
            detection: true,
        })))
    );
    assert_eq!(ask!(handler, SetKeyReports, true), Ok(Some(Ok(()))));
    assert!(HOST.key_reports());
}

#[test]
fn the_host_state_endpoints_are_unsupported_without_host_state() {
    let mut handler = configured();
    assert_eq!(
        ask!(handler, SetKeyReports, false),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, SetGamepadDetection, true),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    assert_eq!(
        ask!(handler, GetGamepadState, ()),
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

#[test]
fn the_switch_and_layout_endpoints_answer_through_the_profile_configurator() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let models = ask!(handler, GetSwitchModels, ());
    assert_eq!(
        models.map(|models| models.map(|models| models.map(|models| models.len()))),
        Ok(Some(Ok(49)))
    );
    assert_eq!(
        ask!(handler, GetSwitches, ()),
        Ok(Some(Ok(factory_switches())))
    );
    let mut tuned = factory_switches();
    tuned.keys[0] = Some(chosen(0, 79));
    assert_eq!(ask!(handler, SetSwitches, tuned.clone()), Ok(Some(Ok(()))));
    assert_eq!(ask!(handler, GetSwitches, ()), Ok(Some(Ok(tuned.clone()))));
    let mut beyond = tuned.clone();
    beyond.keys[0] = Some(chosen(0, 80));
    assert_eq!(
        ask!(handler, SetSwitches, beyond),
        Ok(Some(Err(Rejection::Invalid(
            Invalid::TravelOffsetReachesTravel {
                model: 0,
                offset: 80,
                travel: 80
            }
        ))))
    );
    let mut unknown = tuned.clone();
    unknown.keys[0] = Some(chosen(49, 0));
    assert_eq!(
        ask!(handler, SetSwitches, unknown),
        Ok(Some(Err(Rejection::Invalid(Invalid::SwitchModelUnknown(
            49
        )))))
    );
    assert_eq!(ask!(handler, GetSwitches, ()), Ok(Some(Ok(tuned))));
    assert_eq!(
        ask!(handler, GetLayout, ()),
        Ok(Some(Ok(layouts(Layout::Ansi, Layout::Ansi))))
    );
    assert_eq!(ask!(handler, SetLayout, Layout::Iso), Ok(Some(Ok(()))));
    assert_eq!(
        ask!(handler, SetLayout, Layout::Jis),
        Ok(Some(Err(Rejection::Invalid(Invalid::LayoutMissing(
            Layout::Jis
        )))))
    );
    assert_eq!(
        ask!(handler, GetLayout, ()),
        Ok(Some(Ok(layouts(Layout::Ansi, Layout::Iso))))
    );
    assert_eq!(ask!(handler, Save, ()), Ok(Some(Ok(()))));
    assert_eq!(ask!(handler, Revert, ()), Ok(Some(Ok(()))));
    assert_eq!(
        ask!(handler, GetLayout, ()),
        Ok(Some(Ok(layouts(Layout::Ansi, Layout::Iso))))
    );
}

#[test]
fn the_key_geometry_of_the_layout_in_effect_reaches_the_host() {
    let iso_enter = geometry::Key {
        x: 1350,
        y: 225,
        width: 150,
        height: 200,
        shape: geometry::Shape::IsoEnter,
    };
    let mut ansi = [[None; COLUMNS]; ROWS];
    ansi[2][14] = Some(geometry::Key {
        height: 100,
        shape: geometry::Shape::Rectangle,
        ..iso_enter
    });
    let mut iso = [[None; COLUMNS]; ROWS];
    iso[3][14] = Some(iso_enter);
    let tables = [ansi, iso];
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let model = RefCell::new(open(&keys, &shared, None));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()).with_geometry(&tables),
    );
    let expected: geometry::Keys = ansi.as_flattened().iter().copied().collect();
    assert_eq!(
        ask!(handler, GetKeyGeometry, ()),
        Ok(Some(Ok(expected.clone())))
    );
    assert_eq!(ask!(handler, SetLayout, Layout::Iso), Ok(Some(Ok(()))));
    assert_eq!(
        ask!(handler, GetLayout, ()),
        Ok(Some(Ok(layouts(Layout::Ansi, Layout::Iso))))
    );
    assert_eq!(ask!(handler, GetKeyGeometry, ()), Ok(Some(Ok(expected))));
}

#[test]
fn the_factory_keymap_of_each_layer_reaches_the_host_whatever_the_profiles_hold() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let model = RefCell::new(open(&keys, &shared, None));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let factory: Keymap = Vec::from_array([Action::Keyboard(0x04); ROWS * COLUMNS]);
    let typed: Keymap = Vec::from_array([Action::Keyboard(0x05); ROWS * COLUMNS]);
    let base = LayerRef {
        profile: Slot::new(0),
        layer: Layer::Base,
    };
    assert_eq!(
        ask!(handler, GetDefaultKeymap, Layer::Base),
        Ok(Some(Ok(factory.clone())))
    );
    assert_eq!(
        ask!(
            handler,
            SetKeymap,
            LayerChange {
                profile: base.profile,
                layer: base.layer,
                value: typed.clone(),
            }
        ),
        Ok(Some(Ok(())))
    );
    assert_eq!(ask!(handler, GetKeymap, base), Ok(Some(Ok(typed))));
    assert_eq!(
        ask!(handler, GetDefaultKeymap, Layer::Base),
        Ok(Some(Ok(factory)))
    );
    assert_eq!(
        ask!(handler, GetDefaultKeymap, Layer::Three),
        Ok(Some(Ok(Vec::from_array([Action::None; ROWS * COLUMNS]))))
    );
}

#[test]
fn profiles_swap_and_move_through_the_profile_configurator_and_the_active_one_follows() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    let swap = |first, second| Swap {
        first: Slot::new(first),
        second: Slot::new(second),
    };
    let shift = |from, to| Move {
        from: Slot::new(from),
        to: Slot::new(to),
    };
    let summary = |active| Summary {
        count: 2,
        active: Slot::new(active),
        linked: 0,
        selected: None,
    };
    assert_eq!(ask!(handler, SwapProfiles, swap(0, 1)), Ok(Some(Ok(()))));
    assert_eq!(ask!(handler, GetProfiles, ()), Ok(Some(Ok(summary(1)))));
    assert_eq!(
        ask!(handler, GetProfileInfo, Slot::new(0))
            .map(|reply| reply.map(|info| info.map(|info| info.name))),
        Ok(Some(Ok("Rapid Profile".try_into().unwrap())))
    );
    assert_eq!(ask!(handler, MoveProfile, shift(1, 0)), Ok(Some(Ok(()))));
    assert_eq!(ask!(handler, GetProfiles, ()), Ok(Some(Ok(summary(0)))));
    assert_eq!(
        ask!(handler, GetProfileInfo, Slot::new(0))
            .map(|reply| reply.map(|info| info.map(|info| info.name))),
        Ok(Some(Ok("Typing Profile".try_into().unwrap())))
    );
    assert_eq!(
        ask!(handler, SwapProfiles, swap(0, 3)),
        Ok(Some(Err(Rejection::ProfileMissing(3))))
    );
    assert_eq!(
        ask!(handler, MoveProfile, shift(2, 0)),
        Ok(Some(Err(Rejection::ProfileMissing(2))))
    );
    assert_eq!(
        ask!(handler, SwapProfiles, swap(4, 0)),
        Ok(Some(Err(Rejection::Invalid(Invalid::ProfileOutsideRange(
            4
        )))))
    );
}

#[test]
fn the_storage_usage_reaches_the_host_and_is_unsupported_without_a_store() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&model, switch_capabilities()),
    );
    assert_eq!(
        ask!(handler, GetStorageUsage, ()),
        Ok(Some(Ok(StorageUsage {
            capacity: 4 * SECTOR,
            used: 12,
            records: 1,
            items: 1,
        })))
    );
    let storeless = RefCell::new(open(&keys, &shared, None));
    let mut handler = Handler::with_configuration(
        VERSION,
        stock_identity(None),
        Configurator::new(&storeless, switch_capabilities()),
    );
    assert_eq!(
        ask!(handler, GetStorageUsage, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
    let mut bare = Handler::new(VERSION, stock_identity(None));
    assert_eq!(
        ask!(bare, GetStorageUsage, ()),
        Ok(Some(Err(Rejection::Unsupported)))
    );
}

fn answered<T>(endpoint: &str, answer: &Result<T, Rejection>) {
    assert!(
        !matches!(answer, Err(Rejection::Unsupported)),
        "`{endpoint}` answered unsupported"
    );
}

fn configured_profiles<'a, H: Lightbar>(
    model: &'a RefCell<Model<'a, CriticalSectionRawMutex, Nor, H, ROWS, COLUMNS>>,
) -> Configurator<'a, CriticalSectionRawMutex, Nor, H, ROWS, COLUMNS> {
    static GEOMETRY: [[[Option<geometry::Key>; COLUMNS]; ROWS]; 1] = [[[None; COLUMNS]; ROWS]];
    Configurator::new(
        model,
        Capabilities {
            lightbar_leds: 8,
            host_signals: true,
            ..switch_capabilities()
        },
    )
    .with_geometry(&GEOMETRY)
}

#[test]
fn the_profile_configurator_answers_every_keyboard_wide_endpoint() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = configured_profiles(&model);
    answered("capabilities", &configurator.capabilities());
    let device = configurator.device_settings();
    answered("device", &device);
    answered(
        "device/set",
        &configurator.set_device_settings(device.unwrap().chosen),
    );
    let name = configurator.device_name();
    answered("device/name", &name);
    answered(
        "device/name/set",
        &configurator.set_device_name(name.unwrap()),
    );
    let layout = configurator.layout();
    answered("layout", &layout);
    answered(
        "layout/set",
        &configurator.set_layout(layout.unwrap().chosen),
    );
    answered(
        "geometry",
        &configurator.key_geometry(&mut geometry::Keys::new()),
    );
    answered(
        "default-keymap",
        &configurator.default_keymap(Layer::One, &mut Keymap::new()),
    );
    let global = configurator.global_settings();
    answered("global", &global);
    answered(
        "global/set",
        &configurator.set_global_settings(global.unwrap()),
    );
    let mut entries = lightbar::Entries::new();
    answered("lightbar", &configurator.global_lightbar(&mut entries));
    answered("lightbar/set", &configurator.set_global_lightbar(&entries));
    answered("signals", &configurator.host_signals());
    answered(
        "signals/set",
        &configurator.set_host_signals(lightbar::HostSignals::new()),
    );
    let switches: Result<switches::Settings, Rejection> = configurator.switches();
    answered("switches", &switches);
    answered(
        "switches/set",
        &configurator.set_switches(switches.unwrap()),
    );
    answered("switches/models", &configurator.switch_models());
    answered("profiles", &configurator.profiles());
    answered(
        "profiles/create",
        &configurator.create_profile(openwoot_protocol::profile::Namespace::Onboard),
    );
    answered(
        "profiles/delete",
        &configurator.delete_profile(Slot::new(2)),
    );
    answered(
        "profiles/activate",
        &configurator.activate_profile(Slot::new(1)),
    );
    answered(
        "profiles/swap",
        &configurator.swap_profiles(Slot::new(0), Slot::new(1)),
    );
    answered(
        "profiles/move",
        &configurator.move_profile(Slot::new(1), Slot::new(0)),
    );
    answered("save", &configurator.save());
    answered("revert", &configurator.revert());
    answered("factory-reset", &configurator.factory_reset());
    answered("storage/usage", &configurator.storage_usage());
}

#[test]
fn the_profile_configurator_answers_every_profile_endpoint() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = configured_profiles(&model);
    let slot = Slot::new(0);
    let info = configurator.profile_info(slot);
    answered("profile/info", &info);
    answered(
        "profile/info/set",
        &configurator.set_profile_info(slot, info.unwrap()),
    );
    answered("profile/revision", &configurator.profile_revision(slot));
    let key_settings = configurator.key_settings(slot);
    answered("profile/keys", &key_settings);
    answered(
        "profile/keys/set",
        &configurator.set_key_settings(slot, key_settings.unwrap()),
    );
    let mut keymap = Keymap::new();
    answered(
        "profile/keymap",
        &configurator.keymap(slot, Layer::Base, &mut keymap),
    );
    answered(
        "profile/keymap/set",
        &configurator.set_keymap(slot, Layer::Base, &keymap),
    );
    let mut overrides = openwoot_protocol::keys::Overrides::new();
    answered(
        "profile/overrides",
        &configurator.key_overrides(slot, &mut overrides),
    );
    answered(
        "profile/overrides/set",
        &configurator.set_key_overrides(slot, &overrides),
    );
    let mut advanced = advanced::Keys::new();
    answered(
        "profile/advanced",
        &configurator.advanced_keys(slot, &mut advanced),
    );
    answered(
        "profile/advanced/set",
        &configurator.set_advanced_keys(slot, &advanced),
    );
    let mut mapping = gamepad::Mapping {
        bindings: Vec::new(),
        exclusive: false,
    };
    answered(
        "profile/gamepad/mapping",
        &configurator.gamepad_mapping(slot, &mut mapping),
    );
    answered(
        "profile/gamepad/mapping/set",
        &configurator.set_gamepad_mapping(slot, &mapping),
    );
    let gamepad_settings = configurator.gamepad_settings(slot);
    answered("profile/gamepad", &gamepad_settings);
    answered(
        "profile/gamepad/set",
        &configurator.set_gamepad_settings(slot, gamepad_settings.unwrap()),
    );
    let lighting = configurator.lighting(slot);
    answered("profile/lighting", &lighting);
    answered(
        "profile/lighting/set",
        &configurator.set_lighting(slot, lighting.unwrap()),
    );
    let mut colors = None;
    answered(
        "profile/colors",
        &configurator.key_colors(slot, Layer::Base, &mut colors),
    );
    answered(
        "profile/colors/set",
        &configurator.set_key_colors(slot, Layer::Base, colors.as_ref()),
    );
    let mut lightbar = lightbar::Settings {
        enabled: false,
        entries: lightbar::Entries::new(),
        use_global: true,
        brightness: None,
    };
    answered(
        "profile/lightbar",
        &configurator.lightbar(slot, &mut lightbar),
    );
    answered(
        "profile/lightbar/set",
        &configurator.set_lightbar(slot, &lightbar),
    );
}

struct Raw<'a>(&'a [u8]);

impl Serialize for Raw<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(self.0.len())?;
        for byte in self.0 {
            tuple.serialize_element(byte)?;
        }
        tuple.end()
    }
}

struct Bodies {
    well_formed: std::vec::Vec<u8>,
    malformed: std::vec::Vec<u8>,
}

impl Bodies {
    #[expect(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "every request a test builds fits the largest report"
    )]
    fn of<R: Serialize>(request: &R) -> Self {
        let mut encoded = [0; report::MAX_LEN];
        let length = request.encode(&mut encoded).unwrap();
        let well_formed = encoded[..length].to_vec();
        let malformed = match well_formed.split_last() {
            Some((_, truncated)) => truncated.to_vec(),
            None => vec![0xFF; 4],
        };
        Self {
            well_formed,
            malformed,
        }
    }

    fn truncates(&self) -> bool {
        !self.well_formed.is_empty()
    }
}

type Answered<E> = Result<Option<<E as Endpoint>::Response>, openwoot_protocol::Error>;

#[expect(
    clippy::unwrap_used,
    reason = "every request a test builds fits the largest report and is answered at once"
)]
fn at_once<E, C>(handler: &mut Handler<C>, body: &[u8]) -> Answered<E>
where
    E: Endpoint,
    E::Response: DeserializeOwned,
    C: Configuration,
{
    let header = VarHeader {
        key: VarKey::Key8(E::REQ_KEY),
        seq_no: VarSeq::Seq2(SEQUENCE),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &Raw(body), &mut request).unwrap();
    let mut response = [0; report::MAX_LEN];
    let answer = poll(pin!(handler.respond(request, &mut response))).unwrap();
    assert_eq!(answer.handover, None);
    decode_response::<E>(SEQUENCE, answer.reply.unwrap())
}

fn refused_without_a_light_bar<E, T, C>(handler: &mut Handler<C>, request: &E::Request)
where
    E: Endpoint<Response = Result<T, Rejection>>,
    E::Request: Serialize,
    T: DeserializeOwned + Debug + PartialEq,
    C: Configuration,
{
    let bodies = Bodies::of(request);
    for body in [&bodies.well_formed, &bodies.malformed] {
        assert_eq!(
            at_once::<E, _>(handler, body),
            Ok(Some(Err(Rejection::Unsupported))),
            "`{}` with body {body:?}",
            E::PATH
        );
    }
}

fn served_with_a_light_bar<E, T, C>(handler: &mut Handler<C>, request: &E::Request)
where
    E: Endpoint<Response = Result<T, Rejection>>,
    E::Request: Serialize,
    T: DeserializeOwned + Debug + PartialEq,
    C: Configuration,
{
    let bodies = Bodies::of(request);
    let answer = at_once::<E, _>(handler, &bodies.well_formed);
    assert!(
        matches!(answer, Ok(Some(Ok(_)))),
        "`{}` answered {answer:?}",
        E::PATH
    );
    if bodies.truncates() {
        assert_eq!(
            at_once::<E, _>(handler, &bodies.malformed),
            Err(openwoot_protocol::Error::Device(WireError::DeserFailed)),
            "`{}` with a truncated body",
            E::PATH
        );
    }
}

fn light_bar_handler<'a, H: Lightbar>(
    model: &'a RefCell<Model<'a, CriticalSectionRawMutex, Nor, H, ROWS, COLUMNS>>,
    host_control: &'static HostControl,
) -> Handler<Configurator<'a, CriticalSectionRawMutex, Nor, H, ROWS, COLUMNS>> {
    Handler::with_configuration(VERSION, stock_identity(None), configured_profiles(model))
        .with_host_control(host_control)
}

fn without_light_bar<'a>(
    keys: &'a keys::Shared<CriticalSectionRawMutex>,
    shared: &'a profile::Shared<CriticalSectionRawMutex>,
    settings: &'a mut openwoot::storage::Settings<Nor>,
) -> Model<'a, CriticalSectionRawMutex, Nor, (), ROWS, COLUMNS> {
    let mut model = Model::new(keys, shared, Some(settings), &FACTORY, 0, 4);
    model.load();
    model
}

fn profile_lightbar() -> Change<lightbar::Settings> {
    Change {
        profile: Slot::new(0),
        value: lightbar::Settings {
            enabled: false,
            entries: lightbar::Entries::new(),
            use_global: true,
            brightness: None,
        },
    }
}

fn host_lightbar() -> lightbar::Colors {
    [lighting::Rgb {
        red: 0,
        green: 255,
        blue: 0,
    }]
    .into_iter()
    .collect()
}

#[test]
fn light_bar_endpoints_answer_unsupported_whatever_their_body_on_a_keyboard_without_a_light_bar() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(without_light_bar(&keys, &shared, &mut settings));
    let mut handler = light_bar_handler(&model, &HOST_CONTROL);
    const { assert!(!<Configurator<'_, CriticalSectionRawMutex, Nor, (), ROWS, COLUMNS>>::LIGHTBAR) };
    refused_without_a_light_bar::<GetGlobalLightbar, _, _>(&mut handler, &());
    refused_without_a_light_bar::<SetGlobalLightbar, _, _>(&mut handler, &lightbar::Entries::new());
    refused_without_a_light_bar::<GetHostSignals, _, _>(&mut handler, &());
    refused_without_a_light_bar::<SetHostSignals, _, _>(
        &mut handler,
        &lightbar::HostSignals::new(),
    );
    refused_without_a_light_bar::<GetLightbar, _, _>(&mut handler, &Slot::new(0));
    refused_without_a_light_bar::<SetLightbar, _, _>(&mut handler, &profile_lightbar());
    refused_without_a_light_bar::<ResetHostLightbar, _, _>(&mut handler, &());
    refused_without_a_light_bar::<GetLightbarColors, _, _>(&mut handler, &());
}

#[test]
fn light_bar_endpoints_are_served_on_a_keyboard_with_a_light_bar() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut handler = light_bar_handler::<Bar>(&model, &HOST_CONTROL);
    served_with_a_light_bar::<GetGlobalLightbar, _, _>(&mut handler, &());
    served_with_a_light_bar::<SetGlobalLightbar, _, _>(&mut handler, &lightbar::Entries::new());
    served_with_a_light_bar::<GetHostSignals, _, _>(&mut handler, &());
    served_with_a_light_bar::<SetHostSignals, _, _>(&mut handler, &lightbar::HostSignals::new());
    served_with_a_light_bar::<GetLightbar, _, _>(&mut handler, &Slot::new(0));
    served_with_a_light_bar::<SetLightbar, _, _>(&mut handler, &profile_lightbar());
    served_with_a_light_bar::<ResetHostLightbar, _, _>(&mut handler, &());
    let mut lighting = lit_lighting(&red_bar(true));
    assert!(
        read::<GetLightbarColors, _, _>(&mut handler, &HOST_CONTROL, &mut lighting, START_MS)
            .is_ok()
    );
}

#[test]
fn host_light_bar_colours_are_answered_alike_with_or_without_a_light_bar() {
    static WITHOUT: HostControl = HostControl::new();
    static WITH: HostControl = HostControl::new();
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut bare_settings = storage();
    let bare = RefCell::new(without_light_bar(&keys, &shared, &mut bare_settings));
    let mut without = light_bar_handler(&bare, &WITHOUT);
    let mut settings = storage();
    let model = RefCell::new(open(&keys, &shared, Some(&mut settings)));
    let mut with = light_bar_handler::<Bar>(&model, &WITH);
    let bodies = Bodies::of(&host_lightbar());
    for body in [&bodies.well_formed, &bodies.malformed] {
        assert_eq!(
            at_once::<SetHostLightbar, _>(&mut without, body),
            at_once::<SetHostLightbar, _>(&mut with, body),
            "body {body:?}"
        );
    }
    assert_eq!(
        at_once::<SetHostLightbar, _>(&mut without, &bodies.well_formed),
        Ok(Some(Ok(())))
    );
}
