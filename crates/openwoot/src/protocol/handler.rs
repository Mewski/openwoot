use core::{
    future::Future,
    pin::{Pin, pin},
};

use embedded_hal_async::delay::DelayNs;
use openwoot_protocol::{
    ERROR_KEY, Endpoint, Key, Message, VarKey, Version, WireError, apps,
    configuration::{Rejection, Validate},
    decode_message, encode_message,
    endpoints::{
        ActivateProfile, CreateProfile, DeleteAppLink, DeleteProfile, EnterUpdateMode,
        FactoryReset, GetAdvancedKeys, GetAppLink, GetAppLinkCount, GetCapabilities,
        GetDefaultKeymap, GetDeviceName, GetDeviceSettings, GetDiagnostics, GetGamepadMapping,
        GetGamepadSettings, GetGamepadState, GetGlobalLightbar, GetGlobalSettings, GetHostSignals,
        GetKeyColors, GetKeyGeometry, GetKeyOverrides, GetKeySettings, GetKeymap, GetKeymapLink,
        GetLayout, GetLedFrame, GetLightbar, GetLightbarColors, GetLighting, GetProfileInfo,
        GetProfileRevision, GetProfileState, GetProfiles, GetSensorValues, GetStorageUsage,
        GetSwitchModels, GetSwitches, GetVersion, LeaveHostControl, MoveProfile, ResetHostColors,
        ResetHostKeyColor, ResetHostLightbar, Restart, Revert, Save, SelectLinkedProfile,
        SetAdvancedKeys, SetAppLink, SetDeviceName, SetDeviceSettings, SetGamepadDetection,
        SetGamepadMapping, SetGamepadSettings, SetGlobalLightbar, SetGlobalSettings, SetHostColors,
        SetHostKeyColor, SetHostLightbar, SetHostSignals, SetKeyColors, SetKeyOverrides,
        SetKeyReports, SetKeySettings, SetKeymap, SetKeymapLink, SetLayout, SetLightbar,
        SetLighting, SetProfileInfo, SetRawScanning, SetSwitches, SwapProfiles,
    },
    gamepad, host_lighting,
    keys::{self, Layer},
    lightbar,
    profile::{self, Change, LayerRef, Slot},
    stock::Status,
};
use serde::{Serialize, de::DeserializeOwned};

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
        mut delay: Option<&mut impl DelayNs>,
    ) -> Answer<'a> {
        let message = match decode_message(request) {
            Ok(Some(message)) => message,
            Ok(None) => {
                bounded(delay, self.stock_control().apply_output(request)).await;
                return Answer::NONE;
            }
            Err(_) => return Answer::NONE,
        };
        let VarKey::Key8(key) = message.header.key else {
            let length = fail(&message, &WireError::UnknownKey, response);
            return Answer {
                reply: reply(response, length),
                handover: None,
            };
        };
        let configuration = &mut self.configuration;
        let mut handover = None;
        let mut length = keyboard_endpoints(
            key,
            self.version,
            configuration,
            &message,
            response,
            &mut handover,
        )
        .await;
        if length.is_none() {
            length = profile_endpoints(key, configuration, &message, response).await;
        }
        if length.is_none() {
            length = host_endpoints(key, self.host, configuration, &message, response).await;
        }
        if length.is_none() {
            let served = bounded(
                delay.as_deref_mut(),
                host_control_endpoints(key, self.host_control, configuration, &message, response),
            )
            .await;
            length =
                served.unwrap_or_else(|| Some(fail(&message, &WireError::FailedToSpawn, response)));
        }
        if length.is_none() {
            let served = bounded(
                delay,
                sensor_endpoints(key, self.sensors, configuration, &message, response),
            )
            .await;
            length =
                served.unwrap_or_else(|| Some(fail(&message, &WireError::FailedToSpawn, response)));
        }
        if length.is_none()
            && let Some(readout) = self.diagnostics
        {
            length = diagnostics_endpoint(key, readout, configuration, &message, response).await;
        }
        let length = length.unwrap_or_else(|| fail(&message, &WireError::UnknownKey, response));
        let reply = reply(response, length);
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

impl Answer<'_> {
    pub const NONE: Self = Self {
        reply: None,
        handover: None,
    };
}

macro_rules! dispatch {
    ($key:ident, $configuration:ident, $message:ident, $response:ident;
        $($kind:ident $endpoint:ty => $handle:expr,)*) => {
        $(
            if $key == <$endpoint as Endpoint>::REQ_KEY {
                Some(dispatch!(@$kind $endpoint, $configuration, $message, $response, $handle))
            } else
        )* {
            None
        }
    };
    (@plain $endpoint:ty, $configuration:ident, $message:ident, $response:ident, $handle:expr) => {{
        let served: Pin<&mut dyn Future<Output = Option<usize>>> =
            pin!(serve::<$endpoint, _>($configuration, $message, $response, $handle));
        served.await
    }};
    (@checked $endpoint:ty, $configuration:ident, $message:ident, $response:ident, $handle:expr) => {{
        let served: Pin<&mut dyn Future<Output = Option<usize>>> =
            pin!(serve_checked::<$endpoint, _, _>($configuration, $message, $response, $handle));
        served.await
    }};
    (@filled $endpoint:ty, $configuration:ident, $message:ident, $response:ident, $handle:expr) => {{
        let served: Pin<&mut dyn Future<Output = Option<usize>>> =
            pin!(serve_filled::<$endpoint, _, _>($configuration, $message, $response, $handle));
        served.await
    }};
}

async fn keyboard_endpoints<C: Configuration>(
    key: Key,
    version: Version,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
    handover: &mut Option<Handover>,
) -> Option<Option<usize>> {
    dispatch!(key, configuration, message, response;
        plain GetVersion => async |_, ()| version,
        plain EnterUpdateMode => async |_, ()| {
            *handover = Some(Handover::UpdateMode(HANDOVER_OPTION));
        },
        plain Restart => async |_, ()| {
            *handover = Some(Handover::Restart(HANDOVER_OPTION));
        },
        plain GetCapabilities => async |configuration, ()| configuration.capabilities(),
        plain GetDeviceSettings => async |configuration, ()| configuration.device_settings().await,
        checked SetDeviceSettings => async |configuration, settings| {
            configuration.set_device_settings(settings).await
        },
        plain GetDeviceName => async |configuration, ()| configuration.device_name().await,
        plain SetDeviceName => async |configuration, name| {
            configuration.set_device_name(name).await
        },
        plain GetLayout => async |configuration, ()| configuration.layout().await,
        checked SetLayout => async |configuration, layout| configuration.set_layout(layout).await,
        filled GetKeyGeometry => async |configuration, (), keys| {
            configuration.key_geometry(keys).await
        },
        filled GetDefaultKeymap => async |configuration, layer: Layer, keymap| {
            configuration.default_keymap(layer, keymap).await
        },
        plain GetGlobalSettings => async |configuration, ()| configuration.global_settings().await,
        checked SetGlobalSettings => async |configuration, settings| {
            configuration.set_global_settings(settings).await
        },
        plain GetProfiles => async |configuration, ()| configuration.profiles().await,
        plain CreateProfile => async |configuration, namespace| {
            configuration.create_profile(namespace).await
        },
        checked SelectLinkedProfile => async |configuration, selection| {
            configuration.select_linked_profile(selection).await
        },
        plain GetAppLinkCount => async |configuration, profile: Slot| {
            profile.check_linked(&configuration.capabilities()?)?;
            configuration.app_link_count(profile).await
        },
        filled GetAppLink => async |configuration, link: apps::LinkRef, value| {
            check(configuration, &link)?;
            configuration.app_link(link.profile, link.index, value).await
        },
        checked SetAppLink => async |configuration, change| {
            configuration.set_app_link(change.profile, change.index, &change.link).await
        },
        checked DeleteAppLink => async |configuration, link| {
            configuration.delete_app_link(link.profile, link.index).await
        },
        checked DeleteProfile => async |configuration, profile| {
            configuration.delete_profile(profile).await
        },
        checked ActivateProfile => async |configuration, profile| {
            configuration.activate_profile(profile).await
        },
        checked SwapProfiles => async |configuration, swap| {
            configuration.swap_profiles(swap.first, swap.second).await
        },
        checked MoveProfile => async |configuration, shift| {
            configuration.move_profile(shift.from, shift.to).await
        },
        filled GetGlobalLightbar => async |configuration, (), entries| {
            configuration.global_lightbar(entries).await
        },
        checked SetGlobalLightbar => async |configuration, entries| {
            configuration.set_global_lightbar(&entries).await
        },
        plain GetHostSignals => async |configuration, ()| configuration.host_signals().await,
        checked SetHostSignals => async |configuration, signals| {
            configuration.set_host_signals(signals).await
        },
        plain GetSwitches => async |configuration, ()| configuration.switches().await,
        checked SetSwitches => async |configuration, settings| {
            configuration.set_switches(settings).await
        },
        plain GetSwitchModels => async |configuration, ()| configuration.switch_models().await,
        plain Save => async |configuration, ()| configuration.save().await,
        plain Revert => async |configuration, ()| configuration.revert().await,
        plain FactoryReset => async |configuration, ()| configuration.factory_reset().await,
        plain GetStorageUsage => async |configuration, ()| configuration.storage_usage().await,
    )
}

async fn profile_endpoints<C: Configuration>(
    key: Key,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
) -> Option<Option<usize>> {
    dispatch!(key, configuration, message, response;
        checked GetProfileInfo => async |configuration, profile| {
            configuration.profile_info(profile).await
        },
        checked SetProfileInfo => async |configuration, change| {
            configuration.set_profile_info(change.profile, change.value).await
        },
        checked GetProfileRevision => async |configuration, profile| {
            configuration.profile_revision(profile).await
        },
        checked GetKeySettings => async |configuration, profile| {
            configuration.key_settings(profile).await
        },
        checked SetKeySettings => async |configuration, change| {
            configuration.set_key_settings(change.profile, change.value).await
        },
        filled GetKeymap => async |configuration, layer: LayerRef, keymap| {
            check(configuration, &layer)?;
            configuration.keymap(layer.profile, layer.layer, keymap).await
        },
        checked SetKeymap => async |configuration, change| {
            configuration.set_keymap(change.profile, change.layer, &change.value).await
        },
        checked GetKeymapLink => async |configuration, profile| {
            configuration.keymap_link(profile).await
        },
        plain SetKeymapLink => async |configuration, change: Change<bool>| {
            check(configuration, &change.profile)?;
            configuration.set_keymap_link(change.profile, change.value).await
        },
        filled GetKeyOverrides => async |configuration, profile: Slot, overrides| {
            check(configuration, &profile)?;
            configuration.key_overrides(profile, overrides).await
        },
        checked SetKeyOverrides => async |configuration, change| {
            configuration.set_key_overrides(change.profile, &change.value).await
        },
        filled GetAdvancedKeys => async |configuration, profile: Slot, keys| {
            check(configuration, &profile)?;
            configuration.advanced_keys(profile, keys).await
        },
        checked SetAdvancedKeys => async |configuration, change| {
            configuration.set_advanced_keys(change.profile, &change.value).await
        },
        filled GetGamepadMapping => async |configuration, profile: Slot, mapping| {
            check(configuration, &profile)?;
            configuration.gamepad_mapping(profile, mapping).await
        },
        checked SetGamepadMapping => async |configuration, change| {
            configuration.set_gamepad_mapping(change.profile, &change.value).await
        },
        checked GetGamepadSettings => async |configuration, profile| {
            configuration.gamepad_settings(profile).await
        },
        checked SetGamepadSettings => async |configuration, change| {
            configuration.set_gamepad_settings(change.profile, change.value).await
        },
        checked GetLighting => async |configuration, profile| {
            configuration.lighting(profile).await
        },
        checked SetLighting => async |configuration, change| {
            configuration.set_lighting(change.profile, change.value).await
        },
        filled GetKeyColors => async |configuration, layer: LayerRef, colors| {
            check(configuration, &layer)?;
            configuration.key_colors(layer.profile, layer.layer, colors).await
        },
        checked SetKeyColors => async |configuration, change| {
            configuration
                .set_key_colors(change.profile, change.layer, change.value.as_ref())
                .await
        },
        filled GetLightbar => async |configuration, profile: Slot, settings| {
            check(configuration, &profile)?;
            configuration.lightbar(profile, settings).await
        },
        checked SetLightbar => async |configuration, change| {
            configuration.set_lightbar(change.profile, &change.value).await
        },
    )
}

async fn host_endpoints<C: Configuration>(
    key: Key,
    host: Option<&HostState>,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
) -> Option<Option<usize>> {
    dispatch!(key, configuration, message, response;
        plain SetKeyReports => async |_, enabled| {
            host.map(|host| host.set_key_reports(enabled)).ok_or(Rejection::Unsupported)
        },
        plain GetGamepadState => async |_, ()| {
            host.map(gamepad_state).ok_or(Rejection::Unsupported)
        },
        plain SetGamepadDetection => async |_, enabled| {
            host.map(|host| host.set_gamepad_detection(enabled)).ok_or(Rejection::Unsupported)
        },
    )
}

async fn host_control_endpoints<C: Configuration>(
    key: Key,
    host_control: Option<&HostControl>,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
) -> Option<Option<usize>> {
    let host_control = host_control.ok_or(Rejection::Unsupported);
    dispatch!(key, configuration, message, response;
        checked SetHostKeyColor => async |_, request| {
            host_control?
                .set_key_color(key_at(request.key), request.color)
                .await;
            Ok(())
        },
        checked SetHostColors => async |_, colors| {
            host_control?.set_colors(|keys| keys.clone_from(&colors)).await;
            Ok(())
        },
        checked ResetHostKeyColor => async |_, position| {
            host_control?.reset_key_color(key_at(position)).await;
            Ok(())
        },
        plain ResetHostColors => async |_, ()| {
            host_control?.reset_colors().await;
            Ok(())
        },
        checked SetHostLightbar => async |_, colors| {
            host_control?.set_lightbar(&colors).await;
            Ok(())
        },
        plain ResetHostLightbar => async |configuration, ()| {
            require_lightbar(configuration)?;
            host_control?.reset_lightbar().await;
            Ok(())
        },
        plain LeaveHostControl => async |_, ()| {
            host_control?.leave().await;
            Ok(())
        },
        filled GetLedFrame => async |_, (), frame| {
            host_control?.read_frame(frame).await;
            Ok(())
        },
        filled GetLightbarColors => async |configuration, (), colors| {
            require_lightbar(configuration)?;
            host_control?.read_lightbar(colors).await;
            Ok(())
        },
        plain GetProfileState => async |configuration, ()| {
            let lighting = host_control?;
            let active = configuration.profiles().await?.active;
            let leds = lighting.led_state().await;
            Ok(profile::State {
                active,
                host_control: leds.host_control,
                idle_off: leds.idle_off,
            })
        },
    )
}

async fn sensor_endpoints<C: Configuration>(
    key: Key,
    sensors: Option<&SensorReadout>,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
) -> Option<Option<usize>> {
    let sensors = sensors.ok_or(Rejection::Unsupported);
    dispatch!(key, configuration, message, response;
        filled GetSensorValues => async |_, reading, values| {
            sensors?.read(reading, values).await;
            Ok(())
        },
        plain SetRawScanning => async |_, enabled| {
            sensors.map(|sensors| sensors.set_raw_scanning(enabled))
        },
    )
}

async fn diagnostics_endpoint<C: Configuration>(
    key: Key,
    readout: Readout<'_>,
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
) -> Option<Option<usize>> {
    dispatch!(key, configuration, message, response;
        plain GetDiagnostics => async |_, ()| readout.report(),
    )
}

async fn serve<E, C>(
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
    handle: impl AsyncFnOnce(&mut C, E::Request) -> E::Response,
) -> Option<usize>
where
    E: Endpoint,
    E::Request: DeserializeOwned,
    E::Response: Serialize,
{
    let Some(request) = decode::<E::Request>(message.body) else {
        return fail(message, &WireError::DeserFailed, response);
    };
    let answer = handle(configuration, request).await;
    match encode_message(&message.reply_header(E::RESP_KEY), &answer, response) {
        Ok(encoded) => Some(encoded.len()),
        Err(_) => fail(message, &WireError::SerFailed, response),
    }
}

async fn serve_checked<E, C, T>(
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
    handle: impl AsyncFnOnce(&mut C, E::Request) -> Result<T, Rejection>,
) -> Option<usize>
where
    E: Endpoint<Response = Result<T, Rejection>>,
    E::Request: DeserializeOwned + Validate,
    C: Configuration,
    T: Serialize,
{
    serve::<E, C>(
        configuration,
        message,
        response,
        async |configuration, request| {
            let capabilities = configuration.capabilities()?;
            request.validate(&capabilities)?;
            handle(configuration, request).await
        },
    )
    .await
}

async fn serve_filled<E, C, T>(
    configuration: &mut C,
    message: &Message<'_>,
    response: &mut [u8],
    handle: impl AsyncFnOnce(&mut C, E::Request, &mut T) -> Result<(), Rejection>,
) -> Option<usize>
where
    E: Endpoint<Response = Result<T, Rejection>>,
    E::Request: DeserializeOwned,
    T: Empty + Serialize,
{
    let Some(request) = decode::<E::Request>(message.body) else {
        return fail(message, &WireError::DeserFailed, response);
    };
    let mut value = T::empty();
    let filled = handle(configuration, request, &mut value).await;
    let answer = filled.map(|()| &value);
    match encode_message(&message.reply_header(E::RESP_KEY), &answer, response) {
        Ok(encoded) => Some(encoded.len()),
        Err(_) => fail(message, &WireError::SerFailed, response),
    }
}

fn decode<T: DeserializeOwned>(body: &[u8]) -> Option<T> {
    match postcard::take_from_bytes(body) {
        Ok((request, [])) => Some(request),
        _ => None,
    }
}

fn check<C: Configuration>(configuration: &C, request: &impl Validate) -> Result<(), Rejection> {
    Ok(request.validate(&configuration.capabilities()?)?)
}

fn require_lightbar<C: Configuration>(configuration: &C) -> Result<(), Rejection> {
    if configuration.capabilities()?.lightbar_leds == 0 {
        Err(Rejection::Unsupported)
    } else {
        Ok(())
    }
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

fn fail(message: &Message<'_>, error: &WireError, response: &mut [u8]) -> Option<usize> {
    encode_message(&message.reply_header(ERROR_KEY), error, response)
        .ok()
        .map(<[u8]>::len)
}

fn reply(response: &[u8], length: Option<usize>) -> Option<&[u8]> {
    response.get(..length?)
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
