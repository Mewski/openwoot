use core::future::{Future, ready};

use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced,
    configuration::{Capabilities, Invalid, Rejection},
    device, gamepad, global,
    keys::{self, Keymap, Layer, Overrides},
    lightbar,
    lighting::{self, KeyColors},
    profile::{Info, Slot, Summary},
    switches,
};

use super::Model;
use crate::{keys::switch_models, protocol::Configuration};

pub struct Configurator<'a, M, F, const ROWS: usize, const COLUMNS: usize>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    model: &'a Mutex<M, Model<'a, M, F, ROWS, COLUMNS>>,
    capabilities: Capabilities,
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
        }
    }

    pub async fn serve(&self) -> ! {
        let (keys, shared) = {
            let model = self.model.lock().await;
            (model.keys(), model.shared())
        };
        loop {
            match select(keys.profile_request(), shared.brightness_saved()).await {
                Either::First(profile) => {
                    let _ = self.model.lock().await.activate(Slot::new(profile)).await;
                }
                Either::Second(_) => {
                    let mut model = self.model.lock().await;
                    if shared.take_brightness_save() {
                        let _ = model.save_brightness().await;
                    }
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

    async fn device_settings(&mut self) -> Result<device::Settings, Rejection> {
        Ok(self.model.lock().await.device_settings())
    }

    async fn set_device_settings(&mut self, settings: device::Settings) -> Result<(), Rejection> {
        self.model.lock().await.set_device_settings(settings);
        Ok(())
    }

    async fn layout(&mut self) -> Result<device::Layout, Rejection> {
        let variant = usize::from(self.model.lock().await.variant());
        let layouts = &self.capabilities.layouts;
        layouts
            .get(variant)
            .or(layouts.first())
            .copied()
            .ok_or(Rejection::Unsupported)
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

    async fn create_profile(&mut self) -> Result<Slot, Rejection> {
        self.model.lock().await.create().await
    }

    async fn delete_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model.lock().await.delete(profile).await
    }

    async fn activate_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model.lock().await.activate(profile).await
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
