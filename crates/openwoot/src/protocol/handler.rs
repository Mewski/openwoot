use core::{
    future::poll_fn,
    ops::ControlFlow,
    pin::{Pin, pin},
    task::{Context, Poll},
};

use embedded_hal_async::delay::DelayNs;
use openwoot_protocol::{
    ERROR_KEY, Endpoint, Key, Message, VarKey, Version, WireError, apps,
    configuration::{Capabilities, Rejection, Validate},
    decode_message, encode_frame,
    endpoints::{
        ActivateProfile, ClearMacro, CreateProfile, DeleteAppLink, DeleteProfile, EnterUpdateMode,
        FactoryReset, GetAdvancedKeys, GetAppLink, GetAppLinkCount, GetCapabilities,
        GetDefaultKeymap, GetDeviceName, GetDeviceSettings, GetDiagnostics, GetGamepadMapping,
        GetGamepadSettings, GetGamepadState, GetGlobalLightbar, GetGlobalSettings, GetHostSignals,
        GetKeyColors, GetKeyGeometry, GetKeyOverrides, GetKeySettings, GetKeymap, GetKeymapLink,
        GetLayout, GetLedFrame, GetLightbar, GetLightbarColors, GetLighting, GetMacro, GetMacros,
        GetProfileInfo, GetProfileRevision, GetProfileState, GetProfiles, GetSensorValues,
        GetStorageUsage, GetSwitchModels, GetSwitches, GetVersion, LeaveHostControl, MoveProfile,
        ResetHostColors, ResetHostKeyColor, ResetHostLightbar, Restart, Revert, Save,
        SelectLinkedProfile, SetAdvancedKeys, SetAppLink, SetDeviceName, SetDeviceSettings,
        SetGamepadDetection, SetGamepadMapping, SetGamepadSettings, SetGlobalLightbar,
        SetGlobalSettings, SetHostColors, SetHostKeyColor, SetHostLightbar, SetHostSignals,
        SetKeyColors, SetKeyOverrides, SetKeyReports, SetKeySettings, SetKeymap, SetKeymapLink,
        SetLayout, SetLightbar, SetLighting, SetMacro, SetProfileInfo, SetRawScanning, SetSwitches,
        SwapProfiles,
    },
    gamepad, host_lighting, keys, lightbar, macros,
    profile::{self, Slot},
    stock::Status,
    wire::{self, Encode},
};

use super::{
    Configuration, StockIdentity, Untimed,
    stock::{self, Responder},
    stock_control::{KeyMatrix, StockControl},
    timing::bounded,
};
use crate::{
    diagnostics::Readout,
    lighting::HostControl,
    scan::SensorReadout,
    usb::{FeatureRequest, HostState},
};

const HANDOVER_OPTION: u8 = 0;
const UNSUPPORTED: Result<(), Rejection> = Err(Rejection::Unsupported);
const LIGHTBAR_ENDPOINTS: [(Key, Key); 8] = [
    (GetGlobalLightbar::REQ_KEY, GetGlobalLightbar::RESP_KEY),
    (SetGlobalLightbar::REQ_KEY, SetGlobalLightbar::RESP_KEY),
    (GetHostSignals::REQ_KEY, GetHostSignals::RESP_KEY),
    (SetHostSignals::REQ_KEY, SetHostSignals::RESP_KEY),
    (GetLightbar::REQ_KEY, GetLightbar::RESP_KEY),
    (SetLightbar::REQ_KEY, SetLightbar::RESP_KEY),
    (ResetHostLightbar::REQ_KEY, ResetHostLightbar::RESP_KEY),
    (GetLightbarColors::REQ_KEY, GetLightbarColors::RESP_KEY),
];

#[derive(Debug, Clone, Copy)]
pub struct Handler<C = ()> {
    version: Version,
    stock: Responder,
    configuration: C,
    diagnostics: Option<Readout<'static>>,
    host: Option<&'static HostState>,
    host_control: Option<&'static HostControl>,
    sensors: Option<&'static SensorReadout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer<'a> {
    pub reply: Option<&'a [u8]>,
    pub handover: Option<Handover>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handover {
    UpdateMode(u8),
    Restart(u8),
}

impl Handler {
    #[must_use]
    pub fn new(version: Version, identity: StockIdentity) -> Self {
        Self::with_configuration(version, identity, ())
    }
}

impl<C: Configuration> Handler<C> {
    #[must_use]
    pub fn with_configuration(version: Version, identity: StockIdentity, configuration: C) -> Self {
        Self {
            version,
            stock: Responder::new(identity),
            configuration,
            diagnostics: None,
            host: None,
            host_control: None,
            sensors: None,
        }
    }

    #[must_use]
    pub const fn with_diagnostics(mut self, readout: Readout<'static>) -> Self {
        self.diagnostics = Some(readout);
        self
    }

    #[must_use]
    pub const fn with_host(mut self, host: &'static HostState) -> Self {
        self.host = Some(host);
        self
    }

    #[must_use]
    pub const fn with_host_control(mut self, host_control: &'static HostControl) -> Self {
        self.host_control = Some(host_control);
        self
    }

    #[must_use]
    pub const fn with_sensors(mut self, sensors: &'static SensorReadout) -> Self {
        self.sensors = Some(sensors);
        self
    }

    pub fn configuration(&mut self) -> &mut C {
        &mut self.configuration
    }

    pub async fn respond<'a>(&mut self, request: &[u8], response: &'a mut [u8]) -> Answer<'a> {
        self.respond_within(request, response, None::<&mut Untimed>)
            .await
    }

    pub(super) async fn respond_within<'a>(
        &mut self,
        request: &[u8],
        response: &'a mut [u8],
        delay: Option<&mut impl DelayNs>,
    ) -> Answer<'a> {
        let message = match decode_message(request) {
            Ok(Some(message)) => message,
            Ok(None) => {
                bounded(delay, self.stock_control().apply_output(request)).await;
                return Answer::NONE;
            }
            Err(_) => return Answer::NONE,
        };
        let mut handover = None;
        let mut exchange = Exchange {
            request: message,
            response,
        };
        let length = if let VarKey::Key8(key) = message.header.key {
            self.serve(key, &mut exchange, &mut handover, delay).await
        } else {
            exchange.fail(&WireError::UnknownKey)
        };
        let reply = reply(exchange.response, length);
        Answer {
            reply,
            handover: handover.filter(|_| reply.is_some()),
        }
    }

    pub(super) async fn end_session(&self, delay: Option<&mut impl DelayNs>) {
        if let Some(sensors) = self.sensors {
            sensors.set_raw_scanning(false);
        }
        if let Some(host_control) = self.host_control {
            host_control.end_session();
            bounded(delay, host_control.leave()).await;
        }
    }

    pub(super) async fn answer_stock<'a>(
        &self,
        request: FeatureRequest,
        response: &'a mut [u8],
        delay: Option<&mut impl DelayNs>,
    ) -> Answer<'a> {
        if let FeatureRequest::Command(command) = request {
            match bounded(delay, self.stock_control().answer(command)).await {
                Some(Some(status)) => return stock::reply(command.command, status, &[], response),
                Some(None) => {}
                None => return stock::reply(command.command, Status::Busy, &[], response),
            }
        }
        self.stock.answer(request, response)
    }
}

impl Answer<'_> {
    pub const NONE: Self = Self {
        reply: None,
        handover: None,
    };
}

macro_rules! dispatch {
    ($key:ident, $capabilities:ident, $exchange:ident;
        $($kind:ident $endpoint:ident $(if $feature:ident)? =>
            |$request:pat_param $(, $value:ident)?| $serve:expr,)*) => {
        $(
            if $(C::$feature &&)? $key == <$endpoint as Endpoint>::REQ_KEY {
                return ControlFlow::Break(answered(|| {
                    dispatch!(@serve $kind $endpoint, $capabilities, $exchange,
                        $request, ($($value)?), Ok($serve))
                }));
            }
        )*
    };
    (bounded $key:ident, $capabilities:ident, $exchange:ident, $delay:ident;
        $($kind:ident $endpoint:ident $(if $feature:ident)? =>
            |$request:pat_param $(, $value:ident)?| $serve:expr,)*) => {
        $(
            if $(C::$feature &&)? $key == <$endpoint as Endpoint>::REQ_KEY {
                outlined(pin!(async {
                    dispatch!(@serve $kind $endpoint, $capabilities, $exchange,
                        $request, ($($value)?),
                        bounded($delay, async { $serve })
                            .await
                            .ok_or(WireError::FailedToSpawn))
                }))
                .await
            } else
        )*
        {
            $exchange.fail(&WireError::UnknownKey)
        }
    };
    (@serve $kind:ident $endpoint:ident, $capabilities:ident, $exchange:ident,
        $request:pat, (), $call:expr) => {{
        let answer: Result<<$endpoint as Endpoint>::Response, WireError> =
            match wire::decode::<<$endpoint as Endpoint>::Request>($exchange.request.body) {
                Some(request) => dispatch!(@$kind $capabilities, request, $request, $call),
                None => Err(WireError::DeserFailed),
            };
        $exchange.answer(<$endpoint as Endpoint>::RESP_KEY, reply_body(&answer))
    }};
    (@serve $kind:ident $endpoint:ident, $capabilities:ident, $exchange:ident,
        $request:pat, ($value:ident), $call:expr) => {{
        let mut value = <<$endpoint as Endpoint>::Response as Filled>::Value::empty();
        let filled: Result<Result<(), Rejection>, WireError> =
            match wire::decode::<<$endpoint as Endpoint>::Request>($exchange.request.body) {
                Some(request) => {
                    let $value = &mut value;
                    dispatch!(@$kind $capabilities, request, $request, $call)
                }
                None => Err(WireError::DeserFailed),
            };
        let answer = filled.map(|filled| filled.map(|()| &value));
        $exchange.answer(<$endpoint as Endpoint>::RESP_KEY, reply_body(&answer))
    }};
    (@plain $capabilities:ident, $decoded:ident, $request:pat, $call:expr) => {{
        let $request = $decoded;
        $call
    }};
    (@checked $capabilities:ident, $decoded:ident, $request:pat, $call:expr) => {
        match check(&$capabilities, &$decoded) {
            Ok(()) => {
                let $request = $decoded;
                $call
            }
            Err(rejection) => Ok(Err(rejection)),
        }
    };
}

impl<C: Configuration> Handler<C> {
    async fn serve(
        &mut self,
        key: Key,
        exchange: &mut Exchange<'_, '_>,
        handover: &mut Option<Handover>,
        delay: Option<&mut impl DelayNs>,
    ) -> Option<usize> {
        if let ControlFlow::Break(length) = self.serve_configuration(key, exchange, handover) {
            return length;
        }
        self.serve_real_time(key, exchange, delay).await
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the dispatch table holds one row per endpoint of the protocol"
    )]
    fn serve_configuration(
        &mut self,
        key: Key,
        exchange: &mut Exchange<'_, '_>,
        handover: &mut Option<Handover>,
    ) -> ControlFlow<Option<usize>> {
        let version = self.version;
        let host = self.host;
        let sensors = self.sensors.ok_or(Rejection::Unsupported);
        let configuration = &mut self.configuration;
        let capabilities = configuration.capabilities();
        if key == GetDiagnostics::REQ_KEY {
            let Some(readout) = self.diagnostics else {
                return ControlFlow::Break(exchange.fail(&WireError::UnknownKey));
            };
            return ControlFlow::Break(
                dispatch!(@serve plain GetDiagnostics, capabilities, exchange,
                (), (), Ok(readout.report())),
            );
        }
        if !C::LIGHTBAR
            && let Some(&(_, response)) = LIGHTBAR_ENDPOINTS
                .iter()
                .find(|(request, _)| *request == key)
        {
            return ControlFlow::Break(exchange.answer(response, Ok(&UNSUPPORTED)));
        }
        if key == GetMacro::REQ_KEY {
            return ControlFlow::Break(answered(|| {
                read_macro(configuration, &capabilities, exchange)
            }));
        }
        if key == SetMacro::REQ_KEY {
            return ControlFlow::Break(answered(|| {
                set_macro(configuration, &capabilities, exchange)
            }));
        }
        dispatch!(key, capabilities, exchange;
            plain GetVersion => |()| version,
            plain EnterUpdateMode => |()| *handover = Some(Handover::UpdateMode(HANDOVER_OPTION)),
            plain Restart => |()| *handover = Some(Handover::Restart(HANDOVER_OPTION)),
            plain GetCapabilities => |()| configuration.capabilities(),
            plain GetDeviceSettings => |()| configuration.device_settings(),
            checked SetDeviceSettings => |settings| configuration.set_device_settings(settings),
            plain GetDeviceName => |()| configuration.device_name(),
            plain SetDeviceName => |name| configuration.set_device_name(name),
            plain GetLayout => |()| configuration.layout(),
            checked SetLayout => |layout| configuration.set_layout(layout),
            plain GetKeyGeometry => |(), keys| configuration.key_geometry(keys),
            plain GetDefaultKeymap => |layer, keymap| configuration.default_keymap(layer, keymap),
            plain GetGlobalSettings => |()| configuration.global_settings(),
            checked SetGlobalSettings => |settings| configuration.set_global_settings(settings),
            plain GetProfiles => |()| configuration.profiles(),
            plain CreateProfile => |namespace| configuration.create_profile(namespace),
            checked SelectLinkedProfile => |selection| {
                configuration.select_linked_profile(selection)
            },
            plain GetAppLinkCount => |profile| match linked(&capabilities, profile) {
                Ok(()) => configuration.app_link_count(profile),
                Err(rejection) => Err(rejection),
            },
            checked GetAppLink => |link, value| {
                configuration.app_link(link.profile, link.index, value)
            },
            checked SetAppLink => |change| {
                configuration.set_app_link(change.profile, change.index, &change.link)
            },
            checked DeleteAppLink => |link| configuration.delete_app_link(link.profile, link.index),
            checked DeleteProfile => |profile| configuration.delete_profile(profile),
            checked ActivateProfile => |profile| configuration.activate_profile(profile),
            checked SwapProfiles => |swap| configuration.swap_profiles(swap.first, swap.second),
            checked MoveProfile => |shift| configuration.move_profile(shift.from, shift.to),
            plain GetGlobalLightbar if LIGHTBAR => |(), entries| {
                configuration.global_lightbar(entries)
            },
            checked SetGlobalLightbar if LIGHTBAR => |entries| {
                configuration.set_global_lightbar(&entries)
            },
            plain GetHostSignals if LIGHTBAR => |()| configuration.host_signals(),
            checked SetHostSignals if LIGHTBAR => |signals| configuration.set_host_signals(signals),
            plain GetSwitches => |()| configuration.switches(),
            checked SetSwitches => |settings| configuration.set_switches(settings),
            plain GetSwitchModels => |()| configuration.switch_models(),
            plain Save => |()| configuration.save(),
            plain Revert => |()| configuration.revert(),
            plain FactoryReset => |()| configuration.factory_reset(),
            plain GetStorageUsage => |()| configuration.storage_usage(),
            plain GetMacros => |(), summaries| configuration.macros(summaries),
            plain ClearMacro => |index| match check_macro(&capabilities, index) {
                Ok(()) => configuration.clear_macro(index),
                Err(rejection) => Err(rejection),
            },
            checked GetProfileInfo => |profile| configuration.profile_info(profile),
            checked SetProfileInfo => |change| {
                configuration.set_profile_info(change.profile, change.value)
            },
            checked GetProfileRevision => |profile| configuration.profile_revision(profile),
            checked GetKeySettings => |profile| configuration.key_settings(profile),
            checked SetKeySettings => |change| {
                configuration.set_key_settings(change.profile, change.value)
            },
            checked GetKeymap => |layer, keymap| {
                configuration.keymap(layer.profile, layer.layer, keymap)
            },
            checked SetKeymap => |change| {
                configuration.set_keymap(change.profile, change.layer, &change.value)
            },
            checked GetKeymapLink => |profile| configuration.keymap_link(profile),
            plain SetKeymapLink => |change| match check(&capabilities, &change.profile) {
                Ok(()) => configuration.set_keymap_link(change.profile, change.value),
                Err(rejection) => Err(rejection),
            },
            checked GetKeyOverrides => |profile, overrides| {
                configuration.key_overrides(profile, overrides)
            },
            checked SetKeyOverrides => |change| {
                configuration.set_key_overrides(change.profile, &change.value)
            },
            checked GetAdvancedKeys => |profile, keys| configuration.advanced_keys(profile, keys),
            checked SetAdvancedKeys => |change| {
                configuration.set_advanced_keys(change.profile, &change.value)
            },
            checked GetGamepadMapping => |profile, mapping| {
                configuration.gamepad_mapping(profile, mapping)
            },
            checked SetGamepadMapping => |change| {
                configuration.set_gamepad_mapping(change.profile, &change.value)
            },
            checked GetGamepadSettings => |profile| configuration.gamepad_settings(profile),
            checked SetGamepadSettings => |change| {
                configuration.set_gamepad_settings(change.profile, change.value)
            },
            checked GetLighting => |profile| configuration.lighting(profile),
            checked SetLighting => |change| configuration.set_lighting(change.profile, change.value),
            checked GetKeyColors => |layer, colors| {
                configuration.key_colors(layer.profile, layer.layer, colors)
            },
            checked SetKeyColors => |change| {
                configuration.set_key_colors(change.profile, change.layer, change.value.as_ref())
            },
            checked GetLightbar if LIGHTBAR => |profile, settings| {
                configuration.lightbar(profile, settings)
            },
            checked SetLightbar if LIGHTBAR => |change| {
                configuration.set_lightbar(change.profile, &change.value)
            },
            plain SetKeyReports => |enabled| {
                host.map(|host| host.set_key_reports(enabled)).ok_or(Rejection::Unsupported)
            },
            plain GetGamepadState => |()| host.map(gamepad_state).ok_or(Rejection::Unsupported),
            plain SetGamepadDetection => |enabled| {
                host.map(|host| host.set_gamepad_detection(enabled))
                    .ok_or(Rejection::Unsupported)
            },
            plain SetRawScanning => |enabled| {
                sensors.map(|sensors| sensors.set_raw_scanning(enabled))
            },
        );
        ControlFlow::Continue(())
    }

    async fn serve_real_time(
        &mut self,
        key: Key,
        exchange: &mut Exchange<'_, '_>,
        delay: Option<&mut impl DelayNs>,
    ) -> Option<usize> {
        let host_control = self.host_control.ok_or(Rejection::Unsupported);
        let sensors = self.sensors.ok_or(Rejection::Unsupported);
        let configuration = &mut self.configuration;
        let capabilities = configuration.capabilities();
        dispatch!(bounded key, capabilities, exchange, delay;
            checked SetHostKeyColor => |request| {
                host_control?
                    .set_key_color(key_at(request.key), request.color)
                    .await;
                Ok(())
            },
            checked SetHostColors => |colors| {
                host_control?.set_colors(|keys| keys.clone_from(&colors)).await;
                Ok(())
            },
            checked ResetHostKeyColor => |position| {
                host_control?.reset_key_color(key_at(position)).await;
                Ok(())
            },
            plain ResetHostColors => |()| {
                host_control?.reset_colors().await;
                Ok(())
            },
            checked SetHostLightbar => |colors| {
                host_control?.set_lightbar(&colors).await;
                Ok(())
            },
            plain ResetHostLightbar if LIGHTBAR => |()| {
                require_lightbar(&capabilities)?;
                host_control?.reset_lightbar().await;
                Ok(())
            },
            plain LeaveHostControl => |()| {
                host_control?.leave().await;
                Ok(())
            },
            plain GetLedFrame => |(), frame| {
                host_control?.read_frame(frame).await;
                Ok(())
            },
            plain GetLightbarColors if LIGHTBAR => |(), colors| {
                require_lightbar(&capabilities)?;
                host_control?.read_lightbar(colors).await;
                Ok(())
            },
            plain GetProfileState => |()| {
                let leds = host_control?.led_state().await;
                let active = configuration.profiles()?.active;
                Ok(profile::State {
                    active,
                    host_control: leds.host_control,
                    idle_off: leds.idle_off,
                })
            },
            plain GetSensorValues => |reading, values| {
                sensors?.read(reading, values).await;
                Ok(())
            },
        )
    }

    fn stock_control(&self) -> StockControl<'static> {
        StockControl {
            host: self.host,
            lighting: self.host_control,
            key_matrix: self
                .configuration
                .capabilities()
                .ok()
                .map(|capabilities| KeyMatrix {
                    rows: capabilities.key_rows,
                    columns: capabilities.key_columns,
                }),
        }
    }
}

struct Exchange<'m, 'r> {
    request: Message<'m>,
    response: &'r mut [u8],
}

impl Exchange<'_, '_> {
    fn answer(&mut self, key: Key, answer: Result<&dyn Encode, &WireError>) -> Option<usize> {
        match answer {
            Ok(body) => self
                .encode(key, body)
                .or_else(|| self.fail(&WireError::SerFailed)),
            Err(error) => self.fail(error),
        }
    }

    fn fail(&mut self, error: &WireError) -> Option<usize> {
        self.encode(ERROR_KEY, error)
    }

    fn encode(&mut self, key: Key, body: &dyn Encode) -> Option<usize> {
        encode_frame(&self.request.reply_header(key), body, self.response)
            .ok()
            .map(<[u8]>::len)
    }
}

fn outlined<F: Future>(mut future: Pin<&mut F>) -> impl Future<Output = F::Output> {
    poll_fn(move |context| poll_outlined(future.as_mut(), context))
}

#[inline(never)]
fn poll_outlined<F: Future>(future: Pin<&mut F>, context: &mut Context<'_>) -> Poll<F::Output> {
    future.poll(context)
}

#[inline(never)]
fn answered<R>(serve: impl FnOnce() -> R) -> R {
    serve()
}

fn reply_body<T: Encode>(answer: &Result<T, WireError>) -> Result<&dyn Encode, &WireError> {
    answer.as_ref().map(|answer| answer as &dyn Encode)
}

fn reply(response: &[u8], length: Option<usize>) -> Option<&[u8]> {
    response.get(..length?)
}

fn check(
    capabilities: &Result<Capabilities, Rejection>,
    request: &dyn Validate,
) -> Result<(), Rejection> {
    Ok(request.validate(capabilities.as_ref().map_err(|rejection| *rejection)?)?)
}

fn linked(capabilities: &Result<Capabilities, Rejection>, profile: Slot) -> Result<(), Rejection> {
    Ok(profile.check_linked(capabilities.as_ref().map_err(|rejection| *rejection)?)?)
}

fn check_macro(capabilities: &Result<Capabilities, Rejection>, index: u8) -> Result<(), Rejection> {
    capabilities.as_ref().map_err(|rejection| *rejection)?;
    Ok(macros::check_index(index)?)
}

fn read_macro<C: Configuration>(
    configuration: &mut C,
    capabilities: &Result<Capabilities, Rejection>,
    exchange: &mut Exchange<'_, '_>,
) -> Option<usize> {
    let Some(index) = wire::decode::<u8>(exchange.request.body) else {
        return exchange.fail(&WireError::DeserFailed);
    };
    let read = check_macro(capabilities, index).and_then(|()| {
        configuration.read_macro(index, |stored| {
            exchange.answer(GetMacro::RESP_KEY, Ok(&Ok::<_, Rejection>(stored)))
        })
    });
    read.unwrap_or_else(|rejection| {
        let refused: Result<Option<macros::Encoded<'_>>, Rejection> = Err(rejection);
        exchange.answer(GetMacro::RESP_KEY, Ok(&refused))
    })
}

fn set_macro<C: Configuration>(
    configuration: &mut C,
    capabilities: &Result<Capabilities, Rejection>,
    exchange: &mut Exchange<'_, '_>,
) -> Option<usize> {
    let body = exchange.request.body;
    let Some((index, rest)) = wire::take::<u8>(body) else {
        return exchange.fail(&WireError::DeserFailed);
    };
    let Some((value, [])) = macros::Encoded::take(rest) else {
        return exchange.fail(&WireError::DeserFailed);
    };
    let answer = check_macro(capabilities, index)
        .and_then(|()| check(capabilities, &value))
        .and_then(|()| configuration.set_macro(index, value));
    exchange.answer(SetMacro::RESP_KEY, Ok(&answer))
}

fn require_lightbar(capabilities: &Result<Capabilities, Rejection>) -> Result<(), Rejection> {
    if capabilities
        .as_ref()
        .map_err(|rejection| *rejection)?
        .lightbar_leds
        == 0
    {
        Err(Rejection::Unsupported)
    } else {
        Ok(())
    }
}

trait Filled {
    type Value: Empty;
}

impl<T: Empty> Filled for Result<T, Rejection> {
    type Value = T;
}

trait Empty {
    fn empty() -> Self;
}

impl<T, const N: usize> Empty for heapless::Vec<T, N> {
    fn empty() -> Self {
        Self::new()
    }
}

impl<T> Empty for Option<T> {
    fn empty() -> Self {
        None
    }
}

impl Empty for gamepad::Mapping {
    fn empty() -> Self {
        Self {
            bindings: heapless::Vec::new(),
            exclusive: false,
        }
    }
}

impl Empty for lightbar::Settings {
    fn empty() -> Self {
        Self {
            enabled: false,
            entries: heapless::Vec::new(),
            use_global: false,
            brightness: None,
        }
    }
}

impl Empty for apps::Link {
    fn empty() -> Self {
        Self {
            name: apps::Text::new(),
            app: apps::App::Steam(0),
        }
    }
}

impl Empty for host_lighting::Frame {
    fn empty() -> Self {
        Self {
            keys: heapless::Vec::new(),
            lightbar: heapless::Vec::new(),
        }
    }
}

const fn key_at(position: keys::Position) -> crate::keys::Key {
    crate::keys::Key::new(position.row, position.column)
}

fn gamepad_state(host: &HostState) -> gamepad::State {
    gamepad::State {
        player: host
            .xinput_player()
            .and_then(|player| gamepad::Player::ALL.get(usize::from(player)).copied()),
        detection: host.gamepad_detection(),
    }
}
