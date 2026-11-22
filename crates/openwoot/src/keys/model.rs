use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced,
    config::Rejection,
    gamepad,
    keys::{self as config, Keymap, Layer, Overrides},
    profile::{Slot, Summary},
};

use super::{
    ActiveProfile, Keymaps, Parts, Profile, Shared, default_gamepad_settings, default_settings,
};
use crate::{
    layout::DefaultKeymap,
    storage::{Record, Scope, Settings},
};

pub struct Model<'a, M: RawMutex, F: MultiwriteNorFlash, const ROWS: usize, const COLUMNS: usize> {
    shared: &'a Shared<M>,
    storage: Option<Settings<F>>,
    defaults: DefaultKeymap<ROWS, COLUMNS>,
    variant: u8,
    profiles: u8,
    active: u8,
    unsaved: Parts,
}

impl<'a, M, F, const ROWS: usize, const COLUMNS: usize> Model<'a, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    pub async fn open(
        shared: &'a Shared<M>,
        storage: Option<Settings<F>>,
        defaults: DefaultKeymap<ROWS, COLUMNS>,
        variant: u8,
        profiles: u8,
    ) -> Self {
        let mut model = Self {
            shared,
            storage,
            defaults,
            variant,
            profiles: profiles.max(1),
            active: 0,
            unsaved: Parts::NONE,
        };
        let stored = model
            .load::<ActiveProfile>(Scope::Global)
            .await
            .map_or(0, |active| active.0);
        model.active = if stored < model.profiles { stored } else { 0 };
        model.publish().await;
        model
    }

    #[must_use]
    pub const fn shared(&self) -> &'a Shared<M> {
        self.shared
    }

    #[must_use]
    pub const fn summary(&self) -> Summary {
        Summary {
            count: self.profiles,
            active: Slot::new(self.active),
        }
    }

    #[must_use]
    pub const fn unsaved(&self) -> Parts {
        self.unsaved
    }

    pub async fn activate(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(());
        }
        self.save().await?;
        self.active = profile.get();
        self.store(Scope::Global, &ActiveProfile(self.active))
            .await?;
        self.publish().await;
        Ok(())
    }

    pub async fn save(&mut self) -> Result<(), Rejection> {
        let scope = Scope::Profile(self.active);
        let unsaved = self.unsaved;
        if unsaved.contains(Parts::SETTINGS) {
            let part = self.active_part(|profile| profile.settings)?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Parts::KEYMAPS) {
            let part = self.active_part(|profile| profile.keymaps.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Parts::OVERRIDES) {
            let part = self.active_part(|profile| profile.overrides.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Parts::ADVANCED) {
            let part = self.active_part(|profile| profile.advanced.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Parts::GAMEPAD_MAPPING) {
            let part = self.active_part(|profile| profile.gamepad_mapping.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Parts::GAMEPAD_SETTINGS) {
            let part = self.active_part(|profile| profile.gamepad_settings)?;
            self.store(scope, &part).await?;
        }
        self.unsaved = Parts::NONE;
        Ok(())
    }

    pub async fn revert(&mut self) {
        self.publish().await;
    }

    pub async fn factory_reset(&mut self) -> Result<(), Rejection> {
        if let Some(storage) = self.storage.as_mut() {
            storage
                .factory_reset()
                .await
                .map_err(|_| Rejection::Storage)?;
        }
        self.active = 0;
        self.publish().await;
        Ok(())
    }

    pub async fn settings(&mut self, profile: Slot) -> Result<config::Settings, Rejection> {
        self.get(profile, |profile| profile.settings, default_settings)
            .await
    }

    pub async fn set_settings(
        &mut self,
        profile: Slot,
        settings: config::Settings,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::SETTINGS, settings, |profile, settings| {
            profile.settings = settings;
        })
        .await
    }

    pub async fn keymap(&mut self, profile: Slot, layer: Layer) -> Result<Keymap, Rejection> {
        let keymaps = self.keymaps(profile).await?;
        Ok(layer_of(&keymaps, layer))
    }

    pub async fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: Keymap,
    ) -> Result<(), Rejection> {
        let mut keymaps = self.keymaps(profile).await?;
        if let Some(slot) = keymaps.0.get_mut(usize::from(layer.number())) {
            *slot = keymap;
        }
        self.set(profile, Parts::KEYMAPS, keymaps, |profile, keymaps| {
            profile.keymaps = keymaps;
        })
        .await
    }

    pub async fn overrides(&mut self, profile: Slot) -> Result<Overrides, Rejection> {
        self.get(
            profile,
            |profile| profile.overrides.clone(),
            || core::iter::repeat_n(config::Override::default(), ROWS * COLUMNS).collect(),
        )
        .await
    }

    pub async fn set_overrides(
        &mut self,
        profile: Slot,
        overrides: Overrides,
    ) -> Result<(), Rejection> {
        self.set(
            profile,
            Parts::OVERRIDES,
            overrides,
            |profile, overrides| {
                profile.overrides = overrides;
            },
        )
        .await
    }

    pub async fn advanced(&mut self, profile: Slot) -> Result<advanced::Keys, Rejection> {
        self.get(
            profile,
            |profile| profile.advanced.clone(),
            heapless::Vec::new,
        )
        .await
    }

    pub async fn set_advanced(
        &mut self,
        profile: Slot,
        keys: advanced::Keys,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::ADVANCED, keys, |profile, keys| {
            profile.advanced = keys;
        })
        .await
    }

    pub async fn gamepad_mapping(&mut self, profile: Slot) -> Result<gamepad::Mapping, Rejection> {
        self.get(
            profile,
            |profile| profile.gamepad_mapping.clone(),
            || gamepad::Mapping {
                bindings: core::iter::repeat_n(None, ROWS * COLUMNS).collect(),
                exclusive: false,
            },
        )
        .await
    }

    pub async fn set_gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.set(
            profile,
            Parts::GAMEPAD_MAPPING,
            mapping,
            |profile, mapping| {
                profile.gamepad_mapping = mapping;
            },
        )
        .await
    }

    pub async fn gamepad_settings(
        &mut self,
        profile: Slot,
    ) -> Result<gamepad::Settings, Rejection> {
        self.get(
            profile,
            |profile| profile.gamepad_settings,
            default_gamepad_settings,
        )
        .await
    }

    pub async fn set_gamepad_settings(
        &mut self,
        profile: Slot,
        settings: gamepad::Settings,
    ) -> Result<(), Rejection> {
        self.set(
            profile,
            Parts::GAMEPAD_SETTINGS,
            settings,
            |profile, settings| profile.gamepad_settings = settings,
        )
        .await
    }

    async fn keymaps(&mut self, profile: Slot) -> Result<Keymaps, Rejection> {
        let defaults = self.defaults;
        let variant = self.variant;
        self.get(
            profile,
            |profile| profile.keymaps.clone(),
            || default_keymaps(&defaults, variant),
        )
        .await
    }

    async fn publish(&mut self) {
        let profile = self.load_profile(self.active).await;
        self.unsaved = Parts::NONE;
        self.shared.publish(profile, self.profiles, self.active);
    }

    async fn load_profile(&mut self, slot: u8) -> Profile {
        let mut profile = Profile::defaults(&self.defaults, self.variant);
        let scope = Scope::Profile(slot);
        let keys = ROWS * COLUMNS;
        if let Some(settings) = self.load(scope).await {
            profile.settings = settings;
        }
        if let Some(keymaps) = self
            .load::<Keymaps>(scope)
            .await
            .filter(|keymaps| keymaps.0.iter().all(|keymap| keymap.len() == keys))
        {
            profile.keymaps = keymaps;
        }
        if let Some(overrides) = self
            .load::<Overrides>(scope)
            .await
            .filter(|overrides| overrides.len() == keys)
        {
            profile.overrides = overrides;
        }
        if let Some(advanced) = self.load(scope).await {
            profile.advanced = advanced;
        }
        if let Some(mapping) = self
            .load::<gamepad::Mapping>(scope)
            .await
            .filter(|mapping| mapping.bindings.len() == keys)
        {
            profile.gamepad_mapping = mapping;
        }
        if let Some(settings) = self.load(scope).await {
            profile.gamepad_settings = settings;
        }
        profile
    }

    async fn get<T: Record>(
        &mut self,
        profile: Slot,
        from_active: impl FnOnce(&Profile) -> T,
        default: impl FnOnce() -> T,
    ) -> Result<T, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return self.active_part(from_active);
        }
        Ok(self
            .load(Scope::Profile(profile.get()))
            .await
            .unwrap_or_else(default))
    }

    async fn set<T: Record>(
        &mut self,
        profile: Slot,
        parts: Parts,
        value: T,
        into_active: impl FnOnce(&mut Profile, T),
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            return self.store(Scope::Profile(profile.get()), &value).await;
        }
        self.shared
            .change(parts, |profile| into_active(profile, value))
            .ok_or(Rejection::Storage)?;
        self.unsaved = self.unsaved.union(parts);
        Ok(())
    }

    fn active_part<T>(&self, part: impl FnOnce(&Profile) -> T) -> Result<T, Rejection> {
        self.shared.read(part).ok_or(Rejection::Storage)
    }

    const fn check(&self, profile: Slot) -> Result<(), Rejection> {
        if profile.get() < self.profiles {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        }
    }

    async fn load<T: Record>(&mut self, scope: Scope) -> Option<T> {
        self.storage.as_mut()?.load::<T>(scope).await.ok().flatten()
    }

    async fn store<T: Record>(&mut self, scope: Scope, value: &T) -> Result<(), Rejection> {
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::Storage);
        };
        storage
            .store(scope, value)
            .await
            .map_err(|_| Rejection::Storage)
    }
}

impl<M: RawMutex, F: MultiwriteNorFlash, const ROWS: usize, const COLUMNS: usize> core::fmt::Debug
    for Model<'_, M, F, ROWS, COLUMNS>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("profiles", &self.profiles)
            .field("active", &self.active)
            .field("unsaved", &self.unsaved)
            .finish_non_exhaustive()
    }
}

fn default_keymaps<const ROWS: usize, const COLUMNS: usize>(
    defaults: &DefaultKeymap<ROWS, COLUMNS>,
    variant: u8,
) -> Keymaps {
    Keymaps(Layer::ALL.map(|layer| defaults.keymap(layer, variant)))
}

fn layer_of(keymaps: &Keymaps, layer: Layer) -> Keymap {
    keymaps
        .0
        .get(usize::from(layer.number()))
        .cloned()
        .unwrap_or_default()
}
