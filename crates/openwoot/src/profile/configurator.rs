use core::future::{Future, pending, ready};

use embassy_futures::select::{Either3, select3};
use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embassy_time::{Duration, Timer};
use embassy_usb::driver;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced, apps,
    configuration::{Capabilities, Invalid, Rejection},
    device::{self, LayoutState, SettingsState},
    diagnostics::StorageUsage,
    gamepad, geometry, global,
    keys::{self, Keymap, Layer, Overrides},
    lightbar,
    lighting::{self, KeyColors},
    profile::{Info, Namespace, Selection, Slot, Summary},
    switches,
};

use super::Model;
use crate::{keys::switch_models, protocol::Configuration, usb::Speed};

const ACTIVE_PROFILE_SETTLE: Duration = Duration::from_secs(2);

pub struct Configurator<'a, M, F, const ROWS: usize, const COLUMNS: usize>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    model: &'a Mutex<M, Model<'a, M, F, ROWS, COLUMNS>>,
    capabilities: Capabilities,
    geometry: &'a [[[Option<geometry::Key>; COLUMNS]; ROWS]],
    negotiated_speed: Option<fn() -> Option<driver::Speed>>,
}

impl<'a, M, F, const ROWS: usize, const COLUMNS: usize> Configurator<'a, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    #[must_use]
    pub const fn new(
        model: &'a Mutex<M, Model<'a, M, F, ROWS, COLUMNS>>,
        capabilities: Capabilities,
    ) -> Self {
        Self {
            model,
            capabilities,
            geometry: &[],
            negotiated_speed: None,
        }
    }

    #[must_use]
    pub const fn with_geometry(
        mut self,
        geometry: &'a [[[Option<geometry::Key>; COLUMNS]; ROWS]],
    ) -> Self {
        self.geometry = geometry;
        self
    }

    #[must_use]
    pub const fn with_negotiated_speed(
        mut self,
        negotiated_speed: fn() -> Option<driver::Speed>,
    ) -> Self {
        self.negotiated_speed = Some(negotiated_speed);
        self
    }

    pub async fn serve(&self) -> ! {
        let (keys, shared) = {
            let model = self.model.lock().await;
            (model.keys(), model.shared())
        };
        let mut settling = false;
        loop {
            let settle = settling;
            let settled = async move {
                if settle {
                    Timer::after(ACTIVE_PROFILE_SETTLE).await;
                } else {
                    pending::<()>().await;
                }
            };
            match select3(keys.profile_request(), shared.brightness_saved(), settled).await {
                Either3::First(profile) => {
                    let _ = self.model.lock().await.switch(Slot::new(profile)).await;
                    settling = true;
                }
                Either3::Second(_) => {
                    let mut model = self.model.lock().await;
                    if shared.take_brightness_save() {
                        let _ = model.save_brightness().await;
                    }
                }
                Either3::Third(()) => {
                    let _ = self.model.lock().await.store_active().await;
                    settling = false;
                }
            }
        }
    }

    const fn require_lightbar(&self) -> Result<(), Rejection> {
        if self.capabilities.lightbar_leds == 0 {
            Err(Rejection::Unsupported)
        } else {
            Ok(())
        }
    }

    const fn require_host_signals(&self) -> Result<(), Rejection> {
        if self.capabilities.host_signals {
            Ok(())
        } else {
            Err(Rejection::Unsupported)
        }
    }

    fn layout_of(&self, variant: u8) -> Result<device::Layout, Rejection> {
        let layouts = &self.capabilities.layouts;
        layouts
            .get(usize::from(variant))
            .or(layouts.first())
            .copied()
            .ok_or(Rejection::Unsupported)
    }
}

impl<M, F, const ROWS: usize, const COLUMNS: usize> Configuration
    for Configurator<'_, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    fn capabilities(&self) -> Result<Capabilities, Rejection> {
        Ok(self.capabilities.clone())
    }

    async fn device_settings(&mut self) -> Result<SettingsState, Rejection> {
        let (active, chosen) = {
            let model = self.model.lock().await;
            (model.booted_device_settings(), model.device_settings())
        };
        Ok(SettingsState {
            active,
            chosen,
            negotiated: self
                .negotiated_speed
                .and_then(|negotiated| negotiated())
                .map(|negotiated| device::Speed::from(Speed::from(negotiated))),
        })
    }

    async fn set_device_settings(&mut self, settings: device::Settings) -> Result<(), Rejection> {
        self.model.lock().await.set_device_settings(settings);
        Ok(())
    }

    async fn device_name(&mut self) -> Result<device::Name, Rejection> {
        Ok(self.model.lock().await.device_name().await)
    }

    async fn set_device_name(&mut self, name: device::Name) -> Result<(), Rejection> {
        self.model.lock().await.set_device_name(name).await
    }

    async fn layout(&mut self) -> Result<LayoutState, Rejection> {
        let (active, chosen) = {
            let model = self.model.lock().await;
            (model.variant(), model.chosen_variant())
        };
        Ok(LayoutState {
            active: self.layout_of(active)?,
            chosen: self.layout_of(chosen)?,
        })
    }

    async fn set_layout(&mut self, layout: device::Layout) -> Result<(), Rejection> {
        let variant = self
            .capabilities
            .layouts
            .iter()
            .position(|known| *known == layout)
            .and_then(|position| u8::try_from(position).ok())
            .ok_or(Invalid::LayoutMissing(layout))?;
        self.model.lock().await.set_layout_variant(variant);
        Ok(())
    }

    async fn key_geometry(&mut self, keys: &mut geometry::Keys) -> Result<(), Rejection> {
        let variant = usize::from(self.model.lock().await.variant());
        let table = self
            .geometry
            .get(variant)
            .or(self.geometry.first())
            .ok_or(Rejection::Unsupported)?;
        keys.clear();
        keys.extend_from_slice(table.as_flattened())
            .map_err(|_| Rejection::Unsupported)
    }

    async fn default_keymap(&mut self, layer: Layer, keymap: &mut Keymap) -> Result<(), Rejection> {
        self.model.lock().await.default_keymap(layer, keymap);
        Ok(())
    }

    async fn global_settings(&mut self) -> Result<global::Settings, Rejection> {
        Ok(self.model.lock().await.global_settings())
    }

    async fn set_global_settings(&mut self, settings: global::Settings) -> Result<(), Rejection> {
        self.model.lock().await.set_global_settings(settings);
        Ok(())
    }

    async fn profiles(&mut self) -> Result<Summary, Rejection> {
        Ok(self.model.lock().await.summary())
    }

    async fn create_profile(&mut self, namespace: Namespace) -> Result<Slot, Rejection> {
        self.model.lock().await.create(namespace).await
    }

    async fn select_linked_profile(&mut self, selection: Selection) -> Result<(), Rejection> {
        self.model.lock().await.select_linked(selection).await
    }

    async fn app_link_count(&mut self, profile: Slot) -> Result<u8, Rejection> {
        self.model.lock().await.app_link_count(profile).await
    }

    async fn app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &mut apps::Link,
    ) -> Result<(), Rejection> {
        self.model.lock().await.app_link(profile, index, link).await
    }

    async fn set_app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &apps::Link,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_app_link(profile, index, link)
            .await
    }

    async fn delete_app_link(&mut self, profile: Slot, index: u8) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .delete_app_link(profile, index)
            .await
    }

    async fn delete_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model.lock().await.delete(profile).await
    }

    async fn activate_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model.lock().await.activate(profile).await
    }

    async fn swap_profiles(&mut self, first: Slot, second: Slot) -> Result<(), Rejection> {
        self.model.lock().await.swap(first, second).await
    }

    async fn move_profile(&mut self, from: Slot, to: Slot) -> Result<(), Rejection> {
        self.model.lock().await.shift(from, to).await
    }

    async fn profile_info(&mut self, profile: Slot) -> Result<Info, Rejection> {
        self.model.lock().await.info(profile).await
    }

    async fn set_profile_info(&mut self, profile: Slot, info: Info) -> Result<(), Rejection> {
        self.model.lock().await.set_info(profile, info).await
    }

    async fn profile_revision(&mut self, profile: Slot) -> Result<u32, Rejection> {
        self.model.lock().await.revision(profile).await
    }

    async fn key_settings(&mut self, profile: Slot) -> Result<keys::Settings, Rejection> {
        self.model.lock().await.settings(profile).await
    }

    async fn set_key_settings(
        &mut self,
        profile: Slot,
        settings: keys::Settings,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_settings(profile, settings)
            .await
    }

    async fn keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &mut Keymap,
    ) -> Result<(), Rejection> {
        self.model.lock().await.keymap(profile, layer, keymap).await
    }

    async fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &Keymap,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_keymap(profile, layer, keymap)
            .await
    }

    async fn keymap_link(&mut self, profile: Slot) -> Result<bool, Rejection> {
        self.model.lock().await.keymap_link(profile).await
    }

    async fn set_keymap_link(&mut self, profile: Slot, linked: bool) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_keymap_link(profile, linked)
            .await
    }

    async fn key_overrides(
        &mut self,
        profile: Slot,
        overrides: &mut Overrides,
    ) -> Result<(), Rejection> {
        self.model.lock().await.overrides(profile, overrides).await
    }

    async fn set_key_overrides(
        &mut self,
        profile: Slot,
        overrides: &Overrides,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_overrides(profile, overrides)
            .await
    }

    async fn advanced_keys(
        &mut self,
        profile: Slot,
        keys: &mut advanced::Keys,
    ) -> Result<(), Rejection> {
        self.model.lock().await.advanced(profile, keys).await
    }

    async fn set_advanced_keys(
        &mut self,
        profile: Slot,
        keys: &advanced::Keys,
    ) -> Result<(), Rejection> {
        self.model.lock().await.set_advanced(profile, keys).await
    }

    async fn gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &mut gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .gamepad_mapping(profile, mapping)
            .await
    }

    async fn set_gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_gamepad_mapping(profile, mapping)
            .await
    }

    async fn gamepad_settings(&mut self, profile: Slot) -> Result<gamepad::Settings, Rejection> {
        self.model.lock().await.gamepad_settings(profile).await
    }

    async fn set_gamepad_settings(
        &mut self,
        profile: Slot,
        settings: gamepad::Settings,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_gamepad_settings(profile, settings)
            .await
    }

    async fn lighting(&mut self, profile: Slot) -> Result<lighting::Settings, Rejection> {
        self.model.lock().await.lighting(profile).await
    }

    async fn set_lighting(
        &mut self,
        profile: Slot,
        settings: lighting::Settings,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_lighting(profile, settings)
            .await
    }

    async fn key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: &mut Option<KeyColors>,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .key_colors(profile, layer, colors)
            .await
    }

    async fn set_key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: Option<&KeyColors>,
    ) -> Result<(), Rejection> {
        self.model
            .lock()
            .await
            .set_key_colors(profile, layer, colors)
            .await
    }

    async fn lightbar(
        &mut self,
        profile: Slot,
        settings: &mut lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model.lock().await.lightbar(profile, settings).await
    }

    async fn set_lightbar(
        &mut self,
        profile: Slot,
        settings: &lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model
            .lock()
            .await
            .set_lightbar(profile, settings)
            .await
    }

    async fn global_lightbar(&mut self, entries: &mut lightbar::Entries) -> Result<(), Rejection> {
        self.require_lightbar()?;
        entries.clone_from(self.model.lock().await.global_lightbar());
        Ok(())
    }

    async fn set_global_lightbar(&mut self, entries: &lightbar::Entries) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model.lock().await.set_global_lightbar(entries);
        Ok(())
    }

    async fn host_signals(&mut self) -> Result<lightbar::HostSignalIds, Rejection> {
        self.require_host_signals()?;
        let shared = self.model.lock().await.shared();
        Ok(shared.host_signals().ids())
    }

    async fn set_host_signals(&mut self, signals: lightbar::HostSignals) -> Result<(), Rejection> {
        self.require_host_signals()?;
        let shared = self.model.lock().await.shared();
        shared.host_signals().set(&signals);
        Ok(())
    }

    async fn switches(&mut self) -> Result<switches::Settings, Rejection> {
        Ok(self.model.lock().await.switches())
    }

    async fn set_switches(&mut self, settings: switches::Settings) -> Result<(), Rejection> {
        self.model.lock().await.set_switches(&settings)
    }

    fn switch_models(&mut self) -> impl Future<Output = Result<switches::Models, Rejection>> {
        let mut models = switch_models();
        models.truncate(usize::from(self.capabilities.switch_models));
        ready(Ok(models))
    }

    async fn save(&mut self) -> Result<(), Rejection> {
        self.model.lock().await.save().await
    }

    async fn revert(&mut self) -> Result<(), Rejection> {
        self.model.lock().await.revert().await;
        Ok(())
    }

    async fn factory_reset(&mut self) -> Result<(), Rejection> {
        self.model.lock().await.factory_reset().await
    }

    async fn storage_usage(&mut self) -> Result<StorageUsage, Rejection> {
        self.model.lock().await.storage_usage().await
    }
}

impl<M, F, const ROWS: usize, const COLUMNS: usize> core::fmt::Debug
    for Configurator<'_, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Configurator")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}
