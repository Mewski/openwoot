use core::future::{Future, ready};

use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use openwoot::{
    keys,
    profile::{self, Configurator},
    protocol::{Configuration, Handler},
    usb::HostState,
};
use openwoot_protocol::{
    Endpoint, FirmwareVersion, PROTOCOL_VERSION, VarHeader, VarKey, VarSeq, Version, WireError,
    advanced,
    configuration::{Capabilities, Invalid, Rejection},
    decode_response,
    device::Layout,
    encode_message, encode_request,
    endpoints::{
        ActivateProfile, GetCapabilities, GetKeyGeometry, GetKeymap, GetLayout, GetSwitchModels,
        GetSwitches, GetVersion, Save, SetGamepadDetection, SetKeyReports, SetKeymap, SetLayout,
        SetLighting, SetSwitches,
    },
    gamepad, geometry,
    heapless::Vec,
    keys::{Action, Keymap, Layer},
    lightbar,
    lighting::{self, Indicators},
    profile::{Change, LayerChange, LayerRef, Slot},
    report, switches,
};

use crate::{
    lighting::{COLUMNS, ROWS},
    profile::{ProfileModel, chosen, factory_switches, open, storage, switch_capabilities},
    storage::Nor,
    support::stock_identity,
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
    assert_eq!(
        ask!(handler, GetKeyGeometry, ()),
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

#[test]
fn the_switch_and_layout_endpoints_answer_through_the_profile_configurator() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = Mutex::<CriticalSectionRawMutex, _>::new(open(&keys, &shared, Some(&mut settings)));
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
    assert_eq!(ask!(handler, GetLayout, ()), Ok(Some(Ok(Layout::Ansi))));
    assert_eq!(ask!(handler, SetLayout, Layout::Iso), Ok(Some(Ok(()))));
    assert_eq!(
        ask!(handler, SetLayout, Layout::Jis),
        Ok(Some(Err(Rejection::Invalid(Invalid::LayoutMissing(
            Layout::Jis
        )))))
    );
    assert_eq!(ask!(handler, GetLayout, ()), Ok(Some(Ok(Layout::Ansi))));
    assert_eq!(ask!(handler, Save, ()), Ok(Some(Ok(()))));
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
    let model = Mutex::<CriticalSectionRawMutex, _>::new(open(&keys, &shared, None));
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
    assert_eq!(ask!(handler, GetLayout, ()), Ok(Some(Ok(Layout::Ansi))));
    assert_eq!(ask!(handler, GetKeyGeometry, ()), Ok(Some(Ok(expected))));
}

fn answered<T>(endpoint: &str, answer: &Result<T, Rejection>) {
    assert!(
        !matches!(answer, Err(Rejection::Unsupported)),
        "`{endpoint}` answered unsupported"
    );
}

fn configured_profiles<'a>(
    model: &'a Mutex<CriticalSectionRawMutex, ProfileModel<'a>>,
) -> Configurator<'a, CriticalSectionRawMutex, Nor, ROWS, COLUMNS> {
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
    let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = configured_profiles(&model);
    answered("capabilities", &configurator.capabilities());
    let device = block_on(configurator.device_settings());
    answered("device", &device);
    answered(
        "device/set",
        &block_on(configurator.set_device_settings(device.unwrap())),
    );
    let layout = block_on(configurator.layout());
    answered("layout", &layout);
    answered(
        "layout/set",
        &block_on(configurator.set_layout(layout.unwrap())),
    );
    answered(
        "geometry",
        &block_on(configurator.key_geometry(&mut geometry::Keys::new())),
    );
    let global = block_on(configurator.global_settings());
    answered("global", &global);
    answered(
        "global/set",
        &block_on(configurator.set_global_settings(global.unwrap())),
    );
    let mut entries = lightbar::Entries::new();
    answered(
        "lightbar",
        &block_on(configurator.global_lightbar(&mut entries)),
    );
    answered(
        "lightbar/set",
        &block_on(configurator.set_global_lightbar(&entries)),
    );
    answered("signals", &block_on(configurator.host_signals()));
    answered(
        "signals/set",
        &block_on(configurator.set_host_signals(lightbar::HostSignals::new())),
    );
    let switches: Result<switches::Settings, Rejection> = block_on(configurator.switches());
    answered("switches", &switches);
    answered(
        "switches/set",
        &block_on(configurator.set_switches(switches.unwrap())),
    );
    answered("switches/models", &block_on(configurator.switch_models()));
    answered("profiles", &block_on(configurator.profiles()));
    answered("profiles/create", &block_on(configurator.create_profile()));
    answered(
        "profiles/delete",
        &block_on(configurator.delete_profile(Slot::new(2))),
    );
    answered(
        "profiles/activate",
        &block_on(configurator.activate_profile(Slot::new(1))),
    );
    answered("save", &block_on(configurator.save()));
    answered("revert", &block_on(configurator.revert()));
    answered("factory-reset", &block_on(configurator.factory_reset()));
}

#[test]
fn the_profile_configurator_answers_every_profile_endpoint() {
    let keys = keys::Shared::new();
    let shared = profile::Shared::new();
    let mut settings = storage();
    let model = Mutex::new(open(&keys, &shared, Some(&mut settings)));
    let mut configurator = configured_profiles(&model);
    let slot = Slot::new(0);
    let info = block_on(configurator.profile_info(slot));
    answered("profile/info", &info);
    answered(
        "profile/info/set",
        &block_on(configurator.set_profile_info(slot, info.unwrap())),
    );
    answered(
        "profile/revision",
        &block_on(configurator.profile_revision(slot)),
    );
    let key_settings = block_on(configurator.key_settings(slot));
    answered("profile/keys", &key_settings);
    answered(
        "profile/keys/set",
        &block_on(configurator.set_key_settings(slot, key_settings.unwrap())),
    );
    let mut keymap = Keymap::new();
    answered(
        "profile/keymap",
        &block_on(configurator.keymap(slot, Layer::Base, &mut keymap)),
    );
    answered(
        "profile/keymap/set",
        &block_on(configurator.set_keymap(slot, Layer::Base, &keymap)),
    );
    let mut overrides = openwoot_protocol::keys::Overrides::new();
    answered(
        "profile/overrides",
        &block_on(configurator.key_overrides(slot, &mut overrides)),
    );
    answered(
        "profile/overrides/set",
        &block_on(configurator.set_key_overrides(slot, &overrides)),
    );
    let mut advanced = advanced::Keys::new();
    answered(
        "profile/advanced",
        &block_on(configurator.advanced_keys(slot, &mut advanced)),
    );
    answered(
        "profile/advanced/set",
        &block_on(configurator.set_advanced_keys(slot, &advanced)),
    );
    let mut mapping = gamepad::Mapping {
        bindings: Vec::new(),
        exclusive: false,
    };
    answered(
        "profile/gamepad/mapping",
        &block_on(configurator.gamepad_mapping(slot, &mut mapping)),
    );
    answered(
        "profile/gamepad/mapping/set",
        &block_on(configurator.set_gamepad_mapping(slot, &mapping)),
    );
    let gamepad_settings = block_on(configurator.gamepad_settings(slot));
    answered("profile/gamepad", &gamepad_settings);
    answered(
        "profile/gamepad/set",
        &block_on(configurator.set_gamepad_settings(slot, gamepad_settings.unwrap())),
    );
    let lighting = block_on(configurator.lighting(slot));
    answered("profile/lighting", &lighting);
    answered(
        "profile/lighting/set",
        &block_on(configurator.set_lighting(slot, lighting.unwrap())),
    );
    let mut colors = None;
    answered(
        "profile/colors",
        &block_on(configurator.key_colors(slot, Layer::Base, &mut colors)),
    );
    answered(
        "profile/colors/set",
        &block_on(configurator.set_key_colors(slot, Layer::Base, colors.as_ref())),
    );
    let mut lightbar = lightbar::Settings {
        enabled: false,
        entries: lightbar::Entries::new(),
        use_global: true,
        brightness: None,
    };
    answered(
        "profile/lightbar",
        &block_on(configurator.lightbar(slot, &mut lightbar)),
    );
    answered(
        "profile/lightbar/set",
        &block_on(configurator.set_lightbar(slot, &lightbar)),
    );
}
