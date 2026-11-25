use gpui_kit::{AsyncApp, Entity};
use openwoot_protocol::{
    Endpoint, advanced,
    configuration::{Capabilities, Invalid, Rejection, Validate},
    endpoints::{
        GetAdvancedKeys, GetGamepadMapping, GetGamepadSettings, GetKeyColors, GetKeyOverrides,
        GetKeySettings, GetKeymap, GetLightbar, GetLighting, SetAdvancedKeys, SetGamepadMapping,
        SetGamepadSettings, SetKeyColors, SetKeyOverrides, SetKeySettings, SetKeymap, SetLightbar,
        SetLighting,
    },
    gamepad,
    keys::{self, LAYERS, Layer},
    lightbar, lighting,
    profile::{Change, LayerChange, LayerRef, Slot},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    device::Failure,
    store::{Scope, Store},
};

const STEPS_WITHOUT_LIGHTBAR: usize = 6 + 2 * LAYERS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Parts {
    pub(crate) key_settings: keys::Settings,
    pub(crate) keymaps: [keys::Keymap; LAYERS],
    pub(crate) overrides: keys::Overrides,
    pub(crate) advanced_keys: advanced::Keys,
    pub(crate) gamepad_mapping: gamepad::Mapping,
    pub(crate) gamepad_settings: gamepad::Settings,
    pub(crate) lighting: lighting::Settings,
    pub(crate) key_colors: [Option<lighting::KeyColors>; LAYERS],
    pub(crate) lightbar: Option<lightbar::Settings>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("could not take over the {part}: {reason}")]
pub(crate) struct Mismatch {
    part: &'static str,
    reason: Invalid,
}

impl Parts {
    pub(crate) const fn steps(lightbar: bool) -> usize {
        if lightbar {
            STEPS_WITHOUT_LIGHTBAR + 1
        } else {
            STEPS_WITHOUT_LIGHTBAR
        }
    }

    pub(crate) async fn read(
        store: &Entity<Store>,
        profile: Slot,
        lightbar: bool,
        cx: &mut AsyncApp,
        step: &mut impl FnMut(&mut AsyncApp),
    ) -> Result<Self, Failure> {
        let key_settings = fetch::<GetKeySettings, _>(store, profile, cx).await?;
        step(cx);
        let mut keymaps: [keys::Keymap; LAYERS] = Default::default();
        for (layer, keymap) in Layer::ALL.into_iter().zip(&mut keymaps) {
            *keymap = fetch::<GetKeymap, _>(store, LayerRef { profile, layer }, cx).await?;
            step(cx);
        }
        let overrides = fetch::<GetKeyOverrides, _>(store, profile, cx).await?;
        step(cx);
        let advanced_keys = fetch::<GetAdvancedKeys, _>(store, profile, cx).await?;
        step(cx);
        let gamepad_mapping = fetch::<GetGamepadMapping, _>(store, profile, cx).await?;
        step(cx);
        let gamepad_settings = fetch::<GetGamepadSettings, _>(store, profile, cx).await?;
        step(cx);
        let lighting = fetch::<GetLighting, _>(store, profile, cx).await?;
        step(cx);
        let mut key_colors: [Option<lighting::KeyColors>; LAYERS] = Default::default();
        for (layer, colors) in Layer::ALL.into_iter().zip(&mut key_colors) {
            *colors = fetch::<GetKeyColors, _>(store, LayerRef { profile, layer }, cx).await?;
            step(cx);
        }
        let lightbar = if lightbar {
            let settings = fetch::<GetLightbar, _>(store, profile, cx).await?;
            step(cx);
            Some(settings)
        } else {
            None
        };
        Ok(Self {
            key_settings,
            keymaps,
            overrides,
            advanced_keys,
            gamepad_mapping,
            gamepad_settings,
            lighting,
            key_colors,
            lightbar,
        })
    }

    pub(crate) async fn write(
        &self,
        store: &Entity<Store>,
        profile: Slot,
        cx: &mut AsyncApp,
        step: &mut impl FnMut(&mut AsyncApp),
    ) -> Result<(), Failure> {
        let scope = Scope::Profile(profile);
        let key_settings = Change {
            profile,
            value: self.key_settings,
        };
        apply::<SetKeySettings>(store, key_settings, scope, cx).await?;
        step(cx);
        for (layer, keymap) in Layer::ALL.into_iter().zip(&self.keymaps) {
            let value = LayerChange {
                profile,
                layer,
                value: keymap.clone(),
            };
            apply::<SetKeymap>(store, value, scope, cx).await?;
            step(cx);
        }
        let overrides = Change {
            profile,
            value: self.overrides.clone(),
        };
        apply::<SetKeyOverrides>(store, overrides, scope, cx).await?;
        step(cx);
        let advanced_keys = Change {
            profile,
            value: self.advanced_keys.clone(),
        };
        apply::<SetAdvancedKeys>(store, advanced_keys, scope, cx).await?;
        step(cx);
        let gamepad_mapping = Change {
            profile,
            value: self.gamepad_mapping.clone(),
        };
        apply::<SetGamepadMapping>(store, gamepad_mapping, scope, cx).await?;
        step(cx);
        let gamepad_settings = Change {
            profile,
            value: self.gamepad_settings,
        };
        apply::<SetGamepadSettings>(store, gamepad_settings, scope, cx).await?;
        step(cx);
        let lighting = Change {
            profile,
            value: self.lighting.clone(),
        };
        apply::<SetLighting>(store, lighting, scope, cx).await?;
        step(cx);
        for (layer, colors) in Layer::ALL.into_iter().zip(&self.key_colors) {
            let value = LayerChange {
                profile,
                layer,
                value: colors.clone(),
            };
            apply::<SetKeyColors>(store, value, scope, cx).await?;
            step(cx);
        }
        if let Some(lightbar) = &self.lightbar {
            let value = Change {
                profile,
                value: lightbar.clone(),
            };
            apply::<SetLightbar>(store, value, scope, cx).await?;
            step(cx);
        }
        Ok(())
    }

    pub(crate) fn check(&self, capabilities: &Capabilities) -> Result<(), Mismatch> {
        fits("key settings", &self.key_settings, capabilities)?;
        self.keymaps
            .iter()
            .try_for_each(|keymap| fits("key bindings", keymap, capabilities))?;
        fits("per-key settings", &self.overrides, capabilities)?;
        fits("advanced keys", &self.advanced_keys, capabilities)?;
        fits("gamepad bindings", &self.gamepad_mapping, capabilities)?;
        fits("gamepad settings", &self.gamepad_settings, capabilities)?;
        fits("lighting", &self.lighting, capabilities)?;
        self.key_colors
            .iter()
            .try_for_each(|colors| fits("key colours", colors, capabilities))?;
        fits("light bar settings", &self.lightbar, capabilities)
    }
}

pub(crate) async fn fetch<E, T>(
    store: &Entity<Store>,
    request: E::Request,
    cx: &mut AsyncApp,
) -> Result<T, Failure>
where
    E: Endpoint<Response = Result<T, Rejection>>,
    E::Request: Serialize + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    store
        .update(cx, |store, cx| store.fetch::<E, T>(request, cx))
        .await
}

pub(crate) async fn apply<E>(
    store: &Entity<Store>,
    request: E::Request,
    scope: Scope,
    cx: &mut AsyncApp,
) -> Result<(), Failure>
where
    E: Endpoint<Response = Result<(), Rejection>>,
    E::Request: Serialize + Send + 'static,
{
    store
        .update(cx, |store, cx| store.apply::<E>(request, scope, cx))
        .await
}

fn fits(
    part: &'static str,
    value: &impl Validate,
    capabilities: &Capabilities,
) -> Result<(), Mismatch> {
    value
        .validate(capabilities)
        .map_err(|reason| Mismatch { part, reason })
}
