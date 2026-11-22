use core::{
    future::Future,
    pin::{Pin, pin},
};

use openwoot_protocol::{
    ERROR_KEY, Endpoint, Key, Message, VarKey, Version, WireError,
    config::{Rejection, Validate},
    decode_message, encode_message,
    endpoints::{
        ActivateProfile, CreateProfile, DeleteProfile, EnterUpdateMode, FactoryReset,
        GetAdvancedKeys, GetCapabilities, GetDeviceSettings, GetDiagnostics, GetGamepadMapping,
        GetGamepadSettings, GetGlobalLightbar, GetGlobalSettings, GetHostSignals, GetKeyColors,
        GetKeyOverrides, GetKeySettings, GetKeymap, GetLayout, GetLightbar, GetLighting,
        GetProfileInfo, GetProfileRevision, GetProfiles, GetSwitchModels, GetSwitches, GetVersion,
        Revert, Save, SetAdvancedKeys, SetDeviceSettings, SetGamepadMapping, SetGamepadSettings,
        SetGlobalLightbar, SetGlobalSettings, SetHostSignals, SetKeyColors, SetKeyOverrides,
        SetKeySettings, SetKeymap, SetLayout, SetLightbar, SetLighting, SetProfileInfo,
        SetSwitches,
    },
    gamepad, lightbar,
    profile::{LayerRef, Slot},
};
use serde::{Serialize, de::DeserializeOwned};

use super::{Configuration, StockIdentity, stock::Responder};
use crate::{diagnostics::Readout, usb::FeatureRequest};

const UPDATE_MODE_OPTION: u8 = 0;

#[derive(Debug, Clone, Copy)]
pub struct Handler<C = ()> {
    version: Version,
    stock: Responder,
    configuration: C,
    diagnostics: Option<Readout<'static>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answer<'a> {
    pub reply: Option<&'a [u8]>,
    pub update_mode: Option<u8>,
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
        }
    }

    #[must_use]
    pub const fn with_diagnostics(mut self, readout: Readout<'static>) -> Self {
        self.diagnostics = Some(readout);
        self
    }

    pub fn configuration(&mut self) -> &mut C {
        &mut self.configuration
    }

    pub async fn respond<'a>(&mut self, request: &[u8], response: &'a mut [u8]) -> Answer<'a> {
        let Ok(Some(message)) = decode_message(request) else {
            return Answer::NONE;
        };
        let VarKey::Key8(key) = message.header.key else {
            let len = fail(&message, &WireError::UnknownKey, response);
            return Answer {
                reply: reply(response, len),
                update_mode: None,
            };
        };
        let configuration = &mut self.configuration;
        let mut len =
            keyboard_endpoints(key, self.version, configuration, &message, response).await;
        if len.is_none() {
            len = profile_endpoints(key, configuration, &message, response).await;
        }
        if len.is_none()
            && let Some(readout) = self.diagnostics
        {
            len = diagnostics_endpoint(key, readout, configuration, &message, response).await;
        }
        let len = len.unwrap_or_else(|| fail(&message, &WireError::UnknownKey, response));
        Answer {
            reply: reply(response, len),
            update_mode: (key == EnterUpdateMode::REQ_KEY).then_some(UPDATE_MODE_OPTION),
        }
    }

    pub(super) fn answer_stock<'a>(
        &self,
        request: FeatureRequest,
        response: &'a mut [u8],
    ) -> Answer<'a> {
        self.stock.answer(request, response)
    }
}

impl Answer<'_> {
    pub const NONE: Self = Self {
        reply: None,
        update_mode: None,
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
) -> Option<Option<usize>> {
    dispatch!(key, configuration, message, response;
        plain GetVersion => async |_, ()| version,
        plain EnterUpdateMode => async |_, ()| (),
        plain GetCapabilities => async |configuration, ()| configuration.capabilities(),
        plain GetDeviceSettings => async |configuration, ()| configuration.device_settings().await,
        checked SetDeviceSettings => async |configuration, settings| {
            configuration.set_device_settings(settings).await
        },
        plain GetLayout => async |configuration, ()| configuration.layout().await,
        checked SetLayout => async |configuration, layout| configuration.set_layout(layout).await,
        plain GetGlobalSettings => async |configuration, ()| configuration.global_settings().await,
        checked SetGlobalSettings => async |configuration, settings| {
            configuration.set_global_settings(settings).await
        },
        plain GetProfiles => async |configuration, ()| configuration.profiles().await,
        plain CreateProfile => async |configuration, ()| configuration.create_profile().await,
        checked DeleteProfile => async |configuration, profile| {
            configuration.delete_profile(profile).await
        },
        checked ActivateProfile => async |configuration, profile| {
            configuration.activate_profile(profile).await
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
    let Ok(request) = postcard::from_bytes::<E::Request>(message.body) else {
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
    let Ok(request) = postcard::from_bytes::<E::Request>(message.body) else {
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

fn check<C: Configuration>(configuration: &C, request: &impl Validate) -> Result<(), Rejection> {
    Ok(request.validate(&configuration.capabilities()?)?)
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

fn fail(message: &Message<'_>, error: &WireError, response: &mut [u8]) -> Option<usize> {
    encode_message(&message.reply_header(ERROR_KEY), error, response)
        .ok()
        .map(<[u8]>::len)
}

fn reply(response: &[u8], len: Option<usize>) -> Option<&[u8]> {
    response.get(..len?)
}
