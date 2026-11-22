use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced,
    config::Rejection,
    device, gamepad, global,
    keys::{self as config, Keymap, Layer, Overrides, ScanMode},
    lighting::{self, KeyColors},
    profile::{Info, Slot, Summary},
};

use super::{Appearance, Changes, Colors, Count, Factory, Revision, Shared};
use crate::{
    keys::{self, ActiveProfile, Keymaps, Parts, Profile, default_gamepad_settings},
    storage::{Key, Record, Scope, Settings},
};

pub struct Model<'a, M: RawMutex, F: MultiwriteNorFlash, const ROWS: usize, const COLUMNS: usize> {
    keys: &'a keys::Shared<M>,
    shared: &'a Shared<M>,
    storage: Option<Settings<F>>,
    factory: &'static Factory<ROWS, COLUMNS>,
    variant: u8,
    capacity: u8,
    count: u8,
    active: u8,
    info: Info,
    global: global::Settings,
    device: device::Settings,
    unsaved_keys: Parts,
    unsaved: Changes,
}

impl<'a, M, F, const ROWS: usize, const COLUMNS: usize> Model<'a, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    pub async fn open(
        keys: &'a keys::Shared<M>,
        shared: &'a Shared<M>,
        storage: Option<Settings<F>>,
        factory: &'static Factory<ROWS, COLUMNS>,
        variant: u8,
        capacity: u8,
    ) -> Self {
        let capacity = capacity.max(1);
        let mut model = Self {
            keys,
            shared,
            storage,
            factory,
            variant,
            capacity,
            count: factory.created.clamp(1, capacity),
            active: 0,
            info: factory.preset(0).info(),
            global: factory.global,
            device: device::Settings::from(factory.face),
            unsaved_keys: Parts::NONE,
            unsaved: Changes::NONE,
        };
        model.load_device_wide().await;
        let stored = model
            .load::<ActiveProfile>(Scope::Global)
            .await
            .map_or(0, |active| active.0);
        model.active = if stored < model.count { stored } else { 0 };
        model.publish().await;
        model
    }

    #[must_use]
    pub const fn keys(&self) -> &'a keys::Shared<M> {
        self.keys
    }

    #[must_use]
    pub const fn shared(&self) -> &'a Shared<M> {
        self.shared
    }

    #[must_use]
    pub const fn summary(&self) -> Summary {
        Summary {
            count: self.count,
            active: Slot::new(self.active),
        }
    }

    #[must_use]
    pub const fn unsaved(&self) -> (Parts, Changes) {
        (self.unsaved_keys, self.unsaved)
    }

    pub async fn activate(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(());
        }
        let saved = self.save().await;
        self.active = profile.get();
        self.publish().await;
        let stored = self.store(Scope::Global, &ActiveProfile(self.active)).await;
        saved.and(stored)
    }

    pub async fn create(&mut self) -> Result<Slot, Rejection> {
        if self.count >= self.capacity {
            return Err(Rejection::ProfileLimit);
        }
        let slot = self.count;
        self.remove_profile(slot).await?;
        self.count += 1;
        self.store(Scope::Global, &Count(self.count)).await?;
        self.keys.set_profiles(self.count, self.active);
        Ok(Slot::new(slot))
    }

    pub async fn delete(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        let slot = profile.get();
        if slot == 0 || slot + 1 != self.count {
            return Err(Rejection::ProfileProtected(slot));
        }
        if slot == self.active {
            self.unsaved_keys = Parts::NONE;
            self.unsaved = self.unsaved.intersection(Changes::DEVICE_WIDE);
        }
        self.remove_profile(slot).await?;
        self.count = slot;
        self.store(Scope::Global, &Count(self.count)).await?;
        if self.active >= self.count {
            self.activate(Slot::new(self.count - 1)).await
        } else {
            self.keys.set_profiles(self.count, self.active);
            Ok(())
        }
    }

    pub async fn save(&mut self) -> Result<(), Rejection> {
        let scope = Scope::Profile(self.active);
        let keys = self.unsaved_keys;
        let unsaved = self.unsaved;
        if keys.contains(Parts::SETTINGS) {
            let part = self.active_keys(|profile| profile.settings)?;
            self.store(scope, &part).await?;
        }
        if keys.contains(Parts::KEYMAPS) {
            let part = self.active_keys(|profile| profile.keymaps.clone())?;
            self.store(scope, &part).await?;
        }
        if keys.contains(Parts::OVERRIDES) {
            let part = self.active_keys(|profile| profile.overrides.clone())?;
            self.store(scope, &part).await?;
        }
        if keys.contains(Parts::ADVANCED) {
            let part = self.active_keys(|profile| profile.advanced.clone())?;
            self.store(scope, &part).await?;
        }
        if keys.contains(Parts::GAMEPAD_MAPPING) {
            let part = self.active_keys(|profile| profile.gamepad_mapping.clone())?;
            self.store(scope, &part).await?;
        }
        if keys.contains(Parts::GAMEPAD_SETTINGS) {
            let part = self.active_keys(|profile| profile.gamepad_settings)?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Changes::LIGHTING) {
            let part = self.active_appearance(|appearance| appearance.settings.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Changes::COLORS) {
            let part = self.active_appearance(|appearance| appearance.colors.clone())?;
            self.store(scope, &part).await?;
        }
        if unsaved.contains(Changes::INFO) {
            let info = self.info.clone();
            self.store(scope, &info).await?;
        }
        if unsaved.contains(Changes::GLOBAL) {
            let global = self.global;
            self.store(Scope::Global, &global).await?;
        }
        if unsaved.contains(Changes::DEVICE) {
            let device = self.device;
            self.store(Scope::Global, &device).await?;
        }
        if !keys.is_empty() || !unsaved.without(Changes::DEVICE_WIDE).is_empty() {
            self.bump_revision(self.active).await?;
        }
        self.unsaved_keys = Parts::NONE;
        self.unsaved = Changes::NONE;
        Ok(())
    }

    pub async fn revert(&mut self) {
        self.global = self.factory.global;
        self.device = device::Settings::from(self.factory.face);
        self.load_device_wide().await;
        self.publish().await;
    }

    pub async fn factory_reset(&mut self) -> Result<(), Rejection> {
        if let Some(storage) = self.storage.as_mut() {
            storage
                .factory_reset()
                .await
                .map_err(|_| Rejection::Storage)?;
        }
        self.count = self.factory.created.clamp(1, self.capacity);
        self.active = 0;
        self.global = self.factory.global;
        self.device = device::Settings::from(self.factory.face);
        self.apply_global();
        self.publish().await;
        Ok(())
    }

    pub async fn save_brightness(&mut self) -> Result<(), Rejection> {
        let settings = self.active_appearance(|appearance| appearance.settings.clone())?;
        self.store(Scope::Profile(self.active), &settings).await?;
        self.bump_revision(self.active).await
    }

    #[must_use]
    pub const fn global_settings(&self) -> global::Settings {
        self.global
    }

    pub fn set_global_settings(&mut self, settings: global::Settings) {
        self.global = settings;
        self.apply_global();
        self.unsaved = self.unsaved.union(Changes::GLOBAL);
    }

    #[must_use]
    pub const fn device_settings(&self) -> device::Settings {
        self.device
    }

    pub fn set_device_settings(&mut self, settings: device::Settings) {
        self.device = settings;
        self.unsaved = self.unsaved.union(Changes::DEVICE);
    }

    pub async fn info(&mut self, profile: Slot) -> Result<Info, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(self.info.clone());
        }
        Ok(self.info_of(profile.get()).await)
    }

    pub async fn set_info(&mut self, profile: Slot, info: Info) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            return self.store_part(profile.get(), &info).await;
        }
        self.info = info;
        self.unsaved = self.unsaved.union(Changes::INFO);
        Ok(())
    }

    pub async fn revision(&mut self, profile: Slot) -> Result<u32, Rejection> {
        self.check(profile)?;
        Ok(self
            .load::<Revision>(Scope::Profile(profile.get()))
            .await
            .unwrap_or_default()
            .0)
    }

    pub async fn lighting(&mut self, profile: Slot) -> Result<lighting::Settings, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return self.active_appearance(|appearance| appearance.settings.clone());
        }
        Ok(self.lighting_of(profile.get()).await)
    }

    pub async fn set_lighting(
        &mut self,
        profile: Slot,
        settings: lighting::Settings,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            return self.store_part(profile.get(), &settings).await;
        }
        self.shared
            .change(Changes::LIGHTING, |appearance| {
                appearance.settings = settings;
            })
            .ok_or(Rejection::Storage)?;
        self.unsaved = self.unsaved.union(Changes::LIGHTING);
        Ok(())
    }

    pub async fn key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
    ) -> Result<Option<KeyColors>, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return self.active_appearance(|appearance| appearance.colors.layer(layer).cloned());
        }
        Ok(self.colors_of(profile.get()).await.layer(layer).cloned())
    }

    pub async fn set_key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: Option<KeyColors>,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            let mut stored = self.colors_of(profile.get()).await;
            stored.set(layer, colors);
            return self.store_part(profile.get(), &stored).await;
        }
        self.shared
            .change(Changes::COLORS, |appearance| {
                appearance.colors.set(layer, colors);
            })
            .ok_or(Rejection::Storage)?;
        self.unsaved = self.unsaved.union(Changes::COLORS);
        Ok(())
    }

    pub async fn settings(&mut self, profile: Slot) -> Result<config::Settings, Rejection> {
        let defaults = self.factory.preset(profile.get()).keys;
        self.get(profile, |profile| profile.settings, || defaults)
            .await
    }

    pub async fn set_settings(
        &mut self,
        profile: Slot,
        settings: config::Settings,
    ) -> Result<(), Rejection> {
        let active = profile.get() == self.active;
        self.set(profile, Parts::SETTINGS, settings, |profile, settings| {
            profile.settings = settings;
        })
        .await?;
        if active {
            self.shared
                .set_fast_scan(settings.scan_mode == ScanMode::Parallel);
        }
        Ok(())
    }

    pub async fn keymap(&mut self, profile: Slot, layer: Layer) -> Result<Keymap, Rejection> {
        let keymaps = self.keymaps(profile).await?;
        Ok(keymaps
            .0
            .get(usize::from(layer.number()))
            .cloned()
            .unwrap_or_default())
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
        let defaults = self.default_keymaps();
        self.get(profile, |profile| profile.keymaps.clone(), || defaults)
            .await
    }

    async fn publish(&mut self) {
        let slot = self.active;
        let profile = self.keys_of(slot).await;
        let fast_scan = profile.settings.scan_mode == ScanMode::Parallel;
        self.keys.publish(profile, self.count, slot);
        let settings = self.lighting_of(slot).await;
        let colors = self.colors_of(slot).await;
        self.shared
            .publish(Appearance { settings, colors }, fast_scan);
        self.info = self.info_of(slot).await;
        self.unsaved_keys = Parts::NONE;
        self.unsaved = self.unsaved.intersection(Changes::DEVICE_WIDE);
    }

    async fn load_device_wide(&mut self) {
        if let Some(count) = self.load::<Count>(Scope::Global).await {
            self.count = count.0.clamp(1, self.capacity);
        }
        if let Some(global) = self.load(Scope::Global).await {
            self.global = global;
        }
        if let Some(device) = self.load(Scope::Global).await {
            self.device = device;
        }
        self.unsaved = Changes::NONE;
        self.apply_global();
    }

    fn apply_global(&self) {
        self.shared.set_global(self.global);
        self.keys
            .set_rebound_filter(self.global.release_rebound_filter);
    }

    async fn keys_of(&mut self, slot: u8) -> Profile {
        let mut profile = Profile::defaults(&self.factory.keymap, self.variant);
        profile.settings = self.factory.preset(slot).keys;
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

    async fn lighting_of(&mut self, slot: u8) -> lighting::Settings {
        match self.load(Scope::Profile(slot)).await {
            Some(settings) => settings,
            None => self.factory.preset(slot).lighting.clone(),
        }
    }

    async fn colors_of(&mut self, slot: u8) -> Colors {
        match self.load(Scope::Profile(slot)).await {
            Some(colors) => colors,
            None => Colors(
                self.factory
                    .preset(slot)
                    .colors
                    .map(Option::<&KeyColors>::cloned),
            ),
        }
    }

    async fn info_of(&mut self, slot: u8) -> Info {
        match self.load(Scope::Profile(slot)).await {
            Some(info) => info,
            None => self.factory.preset(slot).info(),
        }
    }

    fn default_keymaps(&self) -> Keymaps {
        Keymaps(Layer::ALL.map(|layer| self.factory.keymap.keymap(layer, self.variant)))
    }

    async fn get<T: Record>(
        &mut self,
        profile: Slot,
        from_active: impl FnOnce(&Profile) -> T,
        default: impl FnOnce() -> T,
    ) -> Result<T, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return self.active_keys(from_active);
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
            return self.store_part(profile.get(), &value).await;
        }
        self.keys
            .change(parts, |profile| into_active(profile, value))
            .ok_or(Rejection::Storage)?;
        self.unsaved_keys = self.unsaved_keys.union(parts);
        Ok(())
    }

    fn active_keys<T>(&self, part: impl FnOnce(&Profile) -> T) -> Result<T, Rejection> {
        self.keys.read(part).ok_or(Rejection::Storage)
    }

    fn active_appearance<T>(&self, part: impl FnOnce(&Appearance) -> T) -> Result<T, Rejection> {
        self.shared.read(part).ok_or(Rejection::Storage)
    }

    const fn check(&self, profile: Slot) -> Result<(), Rejection> {
        if profile.get() < self.count {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        }
    }

    async fn store_part<T: Record>(&mut self, slot: u8, value: &T) -> Result<(), Rejection> {
        self.store(Scope::Profile(slot), value).await?;
        self.bump_revision(slot).await
    }

    async fn bump_revision(&mut self, slot: u8) -> Result<(), Rejection> {
        let scope = Scope::Profile(slot);
        let revision = self.load::<Revision>(scope).await.unwrap_or_default();
        self.store(scope, &Revision(revision.0.wrapping_add(1)))
            .await
    }

    async fn remove_profile(&mut self, slot: u8) -> Result<(), Rejection> {
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::Storage);
        };
        let scope = Scope::Profile(slot);
        for key in [
            Key::of::<config::Settings>(scope),
            Key::of::<Keymaps>(scope),
            Key::of::<Overrides>(scope),
            Key::of::<advanced::Keys>(scope),
            Key::of::<gamepad::Mapping>(scope),
            Key::of::<gamepad::Settings>(scope),
            Key::of::<lighting::Settings>(scope),
            Key::of::<Colors>(scope),
            Key::of::<Info>(scope),
            Key::of::<Revision>(scope),
        ] {
            storage.remove(key).await.map_err(|_| Rejection::Storage)?;
        }
        Ok(())
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
            .field("count", &self.count)
            .field("active", &self.active)
            .field("unsaved_keys", &self.unsaved_keys)
            .field("unsaved", &self.unsaved)
            .finish_non_exhaustive()
    }
}
