use core::{
    cell::{RefCell, RefMut},
    future::pending,
};

use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::RawMutex;
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
use crate::{keys::switch_models, lighting::Lightbar, protocol::Configuration, usb::Speed};

const ACTIVE_PROFILE_SETTLE: Duration = Duration::from_secs(2);

pub struct Configurator<'a, M, F, H, const ROWS: usize, const COLUMNS: usize>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
    H: Lightbar,
{
    model: &'a RefCell<Model<'a, M, F, H, ROWS, COLUMNS>>,
    capabilities: Capabilities,
    geometry: &'a [[[Option<geometry::Key>; COLUMNS]; ROWS]],
    negotiated_speed: Option<fn() -> Option<driver::Speed>>,
}

impl<'a, M, F, H, const ROWS: usize, const COLUMNS: usize> Configurator<'a, M, F, H, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
    H: Lightbar,
{
    #[must_use]
    pub const fn new(
        model: &'a RefCell<Model<'a, M, F, H, ROWS, COLUMNS>>,
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
        let (keys, shared) = match self.model.try_borrow() {
            Ok(model) => (model.keys(), model.shared()),
            Err(_) => pending().await,
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
                    if let Ok(mut model) = self.model() {
                        let _ = model.switch(Slot::new(profile));
                    }
                    settling = true;
                }
                Either3::Second(_) => {
                    if let Ok(mut model) = self.model()
                        && shared.take_brightness_save()
                    {
                        let _ = model.save_brightness();
                    }
                }
                Either3::Third(()) => {
                    if let Ok(mut model) = self.model() {
                        let _ = model.store_active();
                    }
                    settling = false;
                }
            }
        }
    }

    fn model(&self) -> Result<RefMut<'_, Model<'a, M, F, H, ROWS, COLUMNS>>, Rejection> {
        self.model
            .try_borrow_mut()
            .map_err(|_| Rejection::StorageInaccessible)
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

impl<M, F, H, const ROWS: usize, const COLUMNS: usize> Configuration
    for Configurator<'_, M, F, H, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
    H: Lightbar,
{
    const LIGHTBAR: bool = H::PRESENT;

    fn capabilities(&self) -> Result<Capabilities, Rejection> {
        Ok(self.capabilities.clone())
    }

    fn device_settings(&mut self) -> Result<SettingsState, Rejection> {
        let (active, chosen) = {
            let model = self.model()?;
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

    fn set_device_settings(&mut self, settings: device::Settings) -> Result<(), Rejection> {
        self.model()?.set_device_settings(settings);
        Ok(())
    }

    fn device_name(&mut self) -> Result<device::Name, Rejection> {
        Ok(self.model()?.device_name())
    }

    fn set_device_name(&mut self, name: device::Name) -> Result<(), Rejection> {
        self.model()?.set_device_name(name)
    }

    fn layout(&mut self) -> Result<LayoutState, Rejection> {
        let (active, chosen) = {
            let model = self.model()?;
            (model.variant(), model.chosen_variant())
        };
        Ok(LayoutState {
            active: self.layout_of(active)?,
            chosen: self.layout_of(chosen)?,
        })
    }

    fn set_layout(&mut self, layout: device::Layout) -> Result<(), Rejection> {
        let variant = self
            .capabilities
            .layouts
            .iter()
            .position(|known| *known == layout)
            .and_then(|position| u8::try_from(position).ok())
            .ok_or(Invalid::LayoutMissing(layout))?;
        self.model()?.set_layout_variant(variant);
        Ok(())
    }

    fn key_geometry(&mut self, keys: &mut geometry::Keys) -> Result<(), Rejection> {
        let variant = usize::from(self.model()?.variant());
        let table = self
            .geometry
            .get(variant)
            .or(self.geometry.first())
            .ok_or(Rejection::Unsupported)?;
        keys.clear();
        keys.extend_from_slice(table.as_flattened())
            .map_err(|_| Rejection::Unsupported)
    }

    fn default_keymap(&mut self, layer: Layer, keymap: &mut Keymap) -> Result<(), Rejection> {
        self.model()?.default_keymap(layer, keymap);
        Ok(())
    }

    fn global_settings(&mut self) -> Result<global::Settings, Rejection> {
        Ok(self.model()?.global_settings())
    }

    fn set_global_settings(&mut self, settings: global::Settings) -> Result<(), Rejection> {
        self.model()?.set_global_settings(settings);
        Ok(())
    }

    fn profiles(&mut self) -> Result<Summary, Rejection> {
        Ok(self.model()?.summary())
    }

    fn create_profile(&mut self, namespace: Namespace) -> Result<Slot, Rejection> {
        self.model()?.create(namespace)
    }

    fn select_linked_profile(&mut self, selection: Selection) -> Result<(), Rejection> {
        self.model()?.select_linked(selection)
    }

    fn app_link_count(&mut self, profile: Slot) -> Result<u8, Rejection> {
        self.model()?.app_link_count(profile)
    }

    fn app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &mut apps::Link,
    ) -> Result<(), Rejection> {
        self.model()?.app_link(profile, index, link)
    }

    fn set_app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &apps::Link,
    ) -> Result<(), Rejection> {
        self.model()?.set_app_link(profile, index, link)
    }

    fn delete_app_link(&mut self, profile: Slot, index: u8) -> Result<(), Rejection> {
        self.model()?.delete_app_link(profile, index)
    }

    fn delete_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model()?.delete(profile)
    }

    fn activate_profile(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.model()?.activate(profile)
    }

    fn swap_profiles(&mut self, first: Slot, second: Slot) -> Result<(), Rejection> {
        self.model()?.swap(first, second)
    }

    fn move_profile(&mut self, from: Slot, to: Slot) -> Result<(), Rejection> {
        self.model()?.shift(from, to)
    }

    fn profile_info(&mut self, profile: Slot) -> Result<Info, Rejection> {
        self.model()?.info(profile)
    }

    fn set_profile_info(&mut self, profile: Slot, info: Info) -> Result<(), Rejection> {
        self.model()?.set_info(profile, info)
    }

    fn profile_revision(&mut self, profile: Slot) -> Result<u32, Rejection> {
        self.model()?.revision(profile)
    }

    fn key_settings(&mut self, profile: Slot) -> Result<keys::Settings, Rejection> {
        self.model()?.settings(profile)
    }

    fn set_key_settings(
        &mut self,
        profile: Slot,
        settings: keys::Settings,
    ) -> Result<(), Rejection> {
        self.model()?.set_settings(profile, settings)
    }

    fn keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &mut Keymap,
    ) -> Result<(), Rejection> {
        self.model()?.keymap(profile, layer, keymap)
    }

    fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &Keymap,
    ) -> Result<(), Rejection> {
        self.model()?.set_keymap(profile, layer, keymap)
    }

    fn keymap_link(&mut self, profile: Slot) -> Result<bool, Rejection> {
        self.model()?.keymap_link(profile)
    }

    fn set_keymap_link(&mut self, profile: Slot, linked: bool) -> Result<(), Rejection> {
        self.model()?.set_keymap_link(profile, linked)
    }

    fn key_overrides(&mut self, profile: Slot, overrides: &mut Overrides) -> Result<(), Rejection> {
        self.model()?.overrides(profile, overrides)
    }

    fn set_key_overrides(&mut self, profile: Slot, overrides: &Overrides) -> Result<(), Rejection> {
        self.model()?.set_overrides(profile, overrides)
    }

    fn advanced_keys(&mut self, profile: Slot, keys: &mut advanced::Keys) -> Result<(), Rejection> {
        self.model()?.advanced(profile, keys)
    }

    fn set_advanced_keys(&mut self, profile: Slot, keys: &advanced::Keys) -> Result<(), Rejection> {
        self.model()?.set_advanced(profile, keys)
    }

    fn gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &mut gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.model()?.gamepad_mapping(profile, mapping)
    }

    fn set_gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.model()?.set_gamepad_mapping(profile, mapping)
    }

    fn gamepad_settings(&mut self, profile: Slot) -> Result<gamepad::Settings, Rejection> {
        self.model()?.gamepad_settings(profile)
    }

    fn set_gamepad_settings(
        &mut self,
        profile: Slot,
        settings: gamepad::Settings,
    ) -> Result<(), Rejection> {
        self.model()?.set_gamepad_settings(profile, settings)
    }

    fn lighting(&mut self, profile: Slot) -> Result<lighting::Settings, Rejection> {
        self.model()?.lighting(profile)
    }

    fn set_lighting(
        &mut self,
        profile: Slot,
        settings: lighting::Settings,
    ) -> Result<(), Rejection> {
        self.model()?.set_lighting(profile, settings)
    }

    fn key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: &mut Option<KeyColors>,
    ) -> Result<(), Rejection> {
        self.model()?.key_colors(profile, layer, colors)
    }

    fn set_key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: Option<&KeyColors>,
    ) -> Result<(), Rejection> {
        self.model()?.set_key_colors(profile, layer, colors)
    }

    fn lightbar(
        &mut self,
        profile: Slot,
        settings: &mut lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model()?.lightbar(profile, settings)
    }

    fn set_lightbar(
        &mut self,
        profile: Slot,
        settings: &lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model()?.set_lightbar(profile, settings)
    }

    fn global_lightbar(&mut self, entries: &mut lightbar::Entries) -> Result<(), Rejection> {
        self.require_lightbar()?;
        entries.clone_from(self.model()?.global_lightbar());
        Ok(())
    }

    fn set_global_lightbar(&mut self, entries: &lightbar::Entries) -> Result<(), Rejection> {
        self.require_lightbar()?;
        self.model()?.set_global_lightbar(entries);
        Ok(())
    }

    fn host_signals(&mut self) -> Result<lightbar::HostSignalIds, Rejection> {
        self.require_host_signals()?;
        let shared = self.model()?.shared();
        Ok(shared.host_signals().ids())
    }

    fn set_host_signals(&mut self, signals: lightbar::HostSignals) -> Result<(), Rejection> {
        self.require_host_signals()?;
        let shared = self.model()?.shared();
        shared.host_signals().set(&signals);
        Ok(())
    }

    fn switches(&mut self) -> Result<switches::Settings, Rejection> {
        Ok(self.model()?.switches())
    }

    fn set_switches(&mut self, settings: switches::Settings) -> Result<(), Rejection> {
        self.model()?.set_switches(&settings)
    }

    fn switch_models(&mut self) -> Result<switches::Models, Rejection> {
        let mut models = switch_models();
        models.truncate(usize::from(self.capabilities.switch_models));
        Ok(models)
    }

    fn save(&mut self) -> Result<(), Rejection> {
        self.model()?.save()
    }

    fn revert(&mut self) -> Result<(), Rejection> {
        self.model()?.revert();
        Ok(())
    }

    fn factory_reset(&mut self) -> Result<(), Rejection> {
        self.model()?.factory_reset()
    }

    fn storage_usage(&mut self) -> Result<StorageUsage, Rejection> {
        self.model()?.storage_usage()
    }
}

impl<M, F, H, const ROWS: usize, const COLUMNS: usize> core::fmt::Debug
    for Configurator<'_, M, F, H, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
    H: Lightbar,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Configurator")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}
