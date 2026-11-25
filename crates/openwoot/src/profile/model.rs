use core::{
    future::Future,
    pin::{Pin, pin},
};

use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced, apps,
    configuration::{Capabilities, Invalid, MAX_KEYS, Rejection, Validate},
    device,
    diagnostics::StorageUsage,
    gamepad, global,
    keys::{self, Keymap, Layer, Override, Overrides, ScanMode},
    lightbar,
    lighting::{self, KeyColors},
    profile::{Info, MAX_LINKED, Namespace, Selection, Slot, Summary},
    switches,
};
use serde::{Serialize, de::DeserializeOwned};

use super::{
    Appearance, Changes, Colors, Count, DeviceName, Factory, KeyPart, LinkedCount, Order, Preset,
    Revision, Shared,
};
use crate::{
    keys::{ActiveProfile, Keymaps, Parts, Profile, default_gamepad_settings},
    layout::LAYERS,
    lighting::ProfileState,
    storage::{LayoutOverride, Record, RecordKey, Scope, Settings, Stored},
};

const APP_LINK_ITEMS: u8 = 0x30;
const APP_LINK_VERSION: u8 = 1;

pub struct Model<'a, M: RawMutex, F: MultiwriteNorFlash, const ROWS: usize, const COLUMNS: usize> {
    keys: &'a crate::keys::Shared<M>,
    shared: &'a Shared<M>,
    storage: Option<&'a mut Settings<F>>,
    factory: &'static Factory<ROWS, COLUMNS>,
    variant: u8,
    factory_variant: u8,
    capacity: u8,
    count: u8,
    linked_capacity: u8,
    linked: u8,
    selected: Option<u8>,
    returning: u8,
    order: Order,
    active: u8,
    stored_active: u8,
    info: Info,
    global: global::Settings,
    global_lightbar: lightbar::Entries,
    device: device::Settings,
    booted: device::Settings,
    layout: Option<u8>,
    unsaved_keys: Parts,
    unsaved: Changes,
    relink: bool,
}

impl<'a, M, F, const ROWS: usize, const COLUMNS: usize> Model<'a, M, F, ROWS, COLUMNS>
where
    M: RawMutex,
    F: MultiwriteNorFlash,
{
    #[must_use]
    pub fn new(
        keys: &'a crate::keys::Shared<M>,
        shared: &'a Shared<M>,
        storage: Option<&'a mut Settings<F>>,
        factory: &'static Factory<ROWS, COLUMNS>,
        variant: u8,
        capacity: u8,
    ) -> Self {
        const {
            assert!(
                ROWS * COLUMNS <= MAX_KEYS,
                "the key matrix holds more keys than a profile part"
            );
        }
        let capacity = capacity.clamp(1, Order::SLOTS);
        Self {
            keys,
            shared,
            storage,
            factory,
            variant,
            factory_variant: variant,
            capacity,
            count: factory.created.clamp(1, capacity),
            linked_capacity: 0,
            linked: 0,
            selected: None,
            returning: 0,
            order: Order::identity(capacity),
            active: 0,
            stored_active: 0,
            info: factory.preset(0).info(),
            global: factory.global,
            global_lightbar: lightbar::Entries::new(),
            device: device::Settings::from(factory.face),
            booted: device::Settings::from(factory.face),
            layout: None,
            unsaved_keys: Parts::NONE,
            unsaved: Changes::NONE,
            relink: false,
        }
    }

    #[must_use]
    pub const fn with_factory_variant(mut self, variant: u8) -> Self {
        self.factory_variant = variant;
        self
    }

    #[must_use]
    pub fn with_linked(mut self, capacity: u8) -> Self {
        self.linked_capacity = capacity.min(MAX_LINKED);
        self
    }

    pub async fn load(&mut self) {
        self.load_device_wide().await;
        self.booted = self.device;
        let stored = load(&mut self.storage, Scope::Global, |active: ActiveProfile| {
            active.0
        })
        .await
        .unwrap_or(0);
        self.stored_active = stored;
        self.active = self
            .order
            .slot_of(stored)
            .filter(|slot| *slot < self.count)
            .unwrap_or(0);
        self.returning = self.active;
        self.selected = None;
        self.publish().await;
    }

    #[must_use]
    pub const fn keys(&self) -> &'a crate::keys::Shared<M> {
        self.keys
    }

    #[must_use]
    pub const fn shared(&self) -> &'a Shared<M> {
        self.shared
    }

    #[must_use]
    pub const fn variant(&self) -> u8 {
        self.variant
    }

    #[must_use]
    pub fn chosen_variant(&self) -> u8 {
        self.layout.unwrap_or(self.factory_variant)
    }

    #[must_use]
    pub const fn summary(&self) -> Summary {
        Summary {
            count: self.count,
            active: Slot::new(self.active),
            linked: self.linked,
            selected: match self.selected {
                Some(selected) => Some(Slot::new(selected)),
                None => None,
            },
        }
    }

    pub async fn activate(&mut self, profile: Slot) -> Result<(), Rejection> {
        let switched = self.switch(profile).await;
        let stored = self.store_active().await;
        switched.and(stored)
    }

    pub async fn switch(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(());
        }
        let kept = if self.shared.take_brightness_save() {
            self.save_brightness().await
        } else {
            Ok(())
        };
        let saved = kept.and(self.save().await);
        if !profile.is_linked() {
            self.returning = profile.get();
        }
        self.active = profile.get();
        self.publish().await;
        saved
    }

    pub async fn store_active(&mut self) -> Result<(), Rejection> {
        let onboard = if Slot::new(self.active).is_linked() {
            self.returning
        } else {
            self.active
        };
        let slot = self.order.storage_slot(onboard);
        if slot == self.stored_active {
            return Ok(());
        }
        store(&mut self.storage, Scope::Global, &ActiveProfile(slot)).await?;
        self.stored_active = slot;
        Ok(())
    }

    pub async fn create(&mut self, namespace: Namespace) -> Result<Slot, Rejection> {
        let linked = namespace == Namespace::Linked;
        let (count, capacity) = if linked {
            (self.linked, self.linked_capacity)
        } else {
            (self.count, self.capacity)
        };
        if count >= capacity {
            return Err(Rejection::ProfileLimitReached);
        }
        let profile = if linked {
            Slot::linked(count)
        } else {
            Slot::new(count)
        };
        self.remove_profile(profile.get()).await?;
        if linked {
            store(&mut self.storage, Scope::Global, &LinkedCount(count + 1)).await?;
            self.linked = count + 1;
        } else {
            store(&mut self.storage, Scope::Global, &Count(count + 1)).await?;
            self.count = count + 1;
        }
        self.keys.set_profiles(self.count, self.active);
        self.publish_profile_state();
        Ok(profile)
    }

    pub async fn delete(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        let slot = profile.get();
        let (index, count) = if profile.is_linked() {
            (profile.index(), self.linked)
        } else {
            (slot, self.count)
        };
        if slot == 0 || index + 1 != count {
            return Err(Rejection::ProfileProtected(slot));
        }
        if profile.is_linked() {
            store(&mut self.storage, Scope::Global, &LinkedCount(index)).await?;
            self.linked = index;
            if self.selected == Some(slot) {
                self.selected = None;
            }
        } else {
            store(&mut self.storage, Scope::Global, &Count(index)).await?;
            self.count = index;
            self.returning = self.returning.min(index - 1);
        }
        if slot == self.active {
            self.unsaved_keys = Parts::NONE;
            self.unsaved = self.unsaved.intersection(Changes::DEVICE_WIDE);
            self.relink = false;
            let _ = self.shared.take_brightness_save();
        }
        let removed = self.remove_profile(slot).await;
        let handed_over = if slot == self.active {
            let next = if profile.is_linked() {
                self.returning
            } else {
                index - 1
            };
            self.activate(Slot::new(next)).await
        } else {
            self.keys.set_profiles(self.count, self.active);
            self.publish_profile_state();
            Ok(())
        };
        removed.and(handed_over)
    }

    pub async fn select_linked(&mut self, selection: Selection) -> Result<(), Rejection> {
        let target = match selection.profile {
            Some(profile) if !profile.is_linked() => {
                return Err(Invalid::ProfileNotLinked(profile.get()).into());
            }
            Some(profile) => {
                self.check(profile)?;
                self.selected = Some(profile.get());
                (selection.activate || Slot::new(self.active).is_linked()).then_some(profile)
            }
            None => {
                self.selected = None;
                Slot::new(self.active)
                    .is_linked()
                    .then_some(Slot::new(self.returning))
            }
        };
        let activated = match target {
            Some(profile) => self.activate(profile).await,
            None => Ok(()),
        };
        self.publish_profile_state();
        activated
    }

    pub async fn swap(&mut self, first: Slot, second: Slot) -> Result<(), Rejection> {
        self.check_onboard(first)?;
        self.check_onboard(second)?;
        let mut order = self.order.clone();
        order.swap(first.get(), second.get());
        self.reorder(order).await
    }

    pub async fn shift(&mut self, from: Slot, to: Slot) -> Result<(), Rejection> {
        self.check_onboard(from)?;
        self.check_onboard(to)?;
        let mut order = self.order.clone();
        order.shift(from.get(), to.get());
        self.reorder(order).await
    }

    pub async fn save(&mut self) -> Result<(), Rejection> {
        let keys = self.unsaved_keys;
        let unsaved = self.unsaved;
        let relink = self.relink;
        let stored = self.store_unsaved(keys, unsaved).await;
        let relinked = if relink {
            self.remove_keymaps(self.active).await
        } else {
            Ok(())
        };
        let bumped =
            if relink || !keys.is_empty() || !unsaved.without(Changes::DEVICE_WIDE).is_empty() {
                self.bump_revision(self.active).await
            } else {
                Ok(())
            };
        stored.and(relinked).and(bumped)?;
        self.unsaved_keys = Parts::NONE;
        self.unsaved = Changes::NONE;
        self.relink = false;
        Ok(())
    }

    async fn store_unsaved(&mut self, keys: Parts, unsaved: Changes) -> Result<(), Rejection> {
        if keys.contains(Parts::SETTINGS) {
            self.store_keys::<keys::Settings>().await?;
        }
        if keys.contains(Parts::KEYMAPS) {
            self.store_keys::<Keymaps>().await?;
        }
        if keys.contains(Parts::OVERRIDES) {
            self.store_keys::<Overrides>().await?;
        }
        if keys.contains(Parts::ADVANCED) {
            self.store_keys::<advanced::Keys>().await?;
        }
        if keys.contains(Parts::GAMEPAD_MAPPING) {
            self.store_keys::<gamepad::Mapping>().await?;
        }
        if keys.contains(Parts::GAMEPAD_SETTINGS) {
            self.store_keys::<gamepad::Settings>().await?;
        }
        if unsaved.contains(Changes::LIGHTING) {
            self.store_appearance(|appearance| &appearance.settings)
                .await?;
        }
        if unsaved.contains(Changes::COLORS) {
            let shared = self.shared;
            store_with::<_, Colors>(&mut self.storage, self.order.scope(self.active), |body| {
                encode_colors(shared, body)
            })
            .await?;
        }
        if unsaved.contains(Changes::LIGHTBAR) {
            self.store_appearance(|appearance| &appearance.lightbar)
                .await?;
        }
        if unsaved.contains(Changes::INFO) {
            store(&mut self.storage, self.order.scope(self.active), &self.info).await?;
        }
        if unsaved.contains(Changes::GLOBAL) {
            store(&mut self.storage, Scope::Global, &self.global).await?;
        }
        if unsaved.contains(Changes::DEVICE) {
            store(&mut self.storage, Scope::Global, &self.device).await?;
        }
        if unsaved.contains(Changes::GLOBAL_LIGHTBAR) {
            store(&mut self.storage, Scope::Global, &self.global_lightbar).await?;
        }
        if unsaved.contains(Changes::SWITCHES) {
            let keys = self.keys;
            store_with::<_, switches::Settings>(&mut self.storage, Scope::Global, |body| {
                encode_switches(keys, body)
            })
            .await?;
        }
        if unsaved.contains(Changes::LAYOUT)
            && let Some(variant) = self.layout
        {
            store(&mut self.storage, Scope::Global, &LayoutOverride(variant)).await?;
        }
        Ok(())
    }

    pub async fn revert(&mut self) {
        self.load_device_wide().await;
        if self.check(Slot::new(self.active)).is_err() {
            self.active = 0;
        }
        if self.returning >= self.count {
            self.returning = 0;
        }
        self.publish().await;
    }

    pub async fn factory_reset(&mut self) -> Result<(), Rejection> {
        if let Some(storage) = self.storage.as_mut() {
            let erased: Pin<&mut dyn Future<Output = Result<(), crate::storage::Error>>> =
                pin!(storage.factory_reset());
            erased.await.map_err(|_| Rejection::StorageInaccessible)?;
        }
        self.count = self.factory.created.clamp(1, self.capacity);
        self.active = 0;
        self.stored_active = 0;
        self.selected = None;
        self.returning = 0;
        self.load_device_wide().await;
        self.publish().await;
        Ok(())
    }

    pub async fn storage_usage(&mut self) -> Result<StorageUsage, Rejection> {
        let storage = self.storage.as_mut().ok_or(Rejection::Unsupported)?;
        let usage: Pin<&mut dyn Future<Output = Result<StorageUsage, crate::storage::Error>>> =
            pin!(storage.usage());
        usage.await.map_err(|_| Rejection::StorageInaccessible)
    }

    pub async fn app_link_count(&mut self, profile: Slot) -> Result<u8, Rejection> {
        self.check_linked(profile)?;
        let scope = self.order.scope(profile.get());
        let mut count = 0;
        while count < apps::MAX_LINKS
            && load_key(&mut self.storage, app_link_key(scope, count), |_| ())
                .await
                .is_some()
        {
            count += 1;
        }
        Ok(count)
    }

    pub async fn app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &mut apps::Link,
    ) -> Result<(), Rejection> {
        self.check_linked(profile)?;
        let key = app_link_key(self.order.scope(profile.get()), index);
        load_key(&mut self.storage, key, |stored| {
            (stored.version == APP_LINK_VERSION)
                .then(|| postcard::from_bytes::<apps::Link>(stored.body).ok())
                .flatten()
                .map(|stored| *link = stored)
        })
        .await
        .flatten()
        .ok_or(Rejection::AppLinkMissing(index))
    }

    pub async fn set_app_link(
        &mut self,
        profile: Slot,
        index: u8,
        link: &apps::Link,
    ) -> Result<(), Rejection> {
        let count = self.app_link_count(profile).await?;
        if index > count {
            return Err(Rejection::AppLinkMissing(index));
        }
        let key = app_link_key(self.order.scope(profile.get()), index);
        store_app_link(&mut self.storage, key, link).await?;
        self.bump_revision(profile.get()).await
    }

    pub async fn delete_app_link(&mut self, profile: Slot, index: u8) -> Result<(), Rejection> {
        let count = self.app_link_count(profile).await?;
        if index >= count {
            return Err(Rejection::AppLinkMissing(index));
        }
        let scope = self.order.scope(profile.get());
        let mut moved = apps::Link {
            name: apps::Text::new(),
            app: apps::App::Steam(0),
        };
        for next in index + 1..count {
            self.app_link(profile, next, &mut moved).await?;
            store_app_link(&mut self.storage, app_link_key(scope, next - 1), &moved).await?;
        }
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::StorageInaccessible);
        };
        storage
            .remove(app_link_key(scope, count - 1))
            .await
            .map_err(|_| Rejection::StorageInaccessible)?;
        self.bump_revision(profile.get()).await
    }

    pub async fn save_brightness(&mut self) -> Result<(), Rejection> {
        let slot = self.active;
        let level = self
            .shared
            .edit(|appearance| appearance.settings.brightness);
        let limits = self.limits();
        let stored = load_valid(
            &mut self.storage,
            self.order.scope(slot),
            &limits,
            |settings: lighting::Settings| settings,
        )
        .await;
        let mut settings = stored.unwrap_or_else(|| self.preset(slot).lighting.clone());
        if settings.brightness == level {
            return Ok(());
        }
        settings.brightness = level;
        store(&mut self.storage, self.order.scope(slot), &settings).await?;
        self.bump_revision(slot).await
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

    #[must_use]
    pub const fn booted_device_settings(&self) -> device::Settings {
        self.booted
    }

    pub fn set_device_settings(&mut self, settings: device::Settings) {
        self.device = settings;
        self.unsaved = self.unsaved.union(Changes::DEVICE);
    }

    pub async fn device_name(&mut self) -> device::Name {
        load(&mut self.storage, Scope::Global, |name: DeviceName| name.0)
            .await
            .unwrap_or_default()
    }

    pub async fn set_device_name(&mut self, name: device::Name) -> Result<(), Rejection> {
        if self.device_name().await == name {
            return Ok(());
        }
        store(&mut self.storage, Scope::Global, &DeviceName(name)).await
    }

    pub fn set_layout_variant(&mut self, variant: u8) {
        self.layout = Some(variant);
        self.unsaved = self.unsaved.union(Changes::LAYOUT);
    }

    #[must_use]
    pub fn switches(&self) -> switches::Settings {
        self.keys.switches(Clone::clone)
    }

    pub fn set_switches(&mut self, settings: &switches::Settings) -> Result<(), Rejection> {
        self.keys.set_switches(settings)?;
        self.unsaved = self.unsaved.union(Changes::SWITCHES);
        Ok(())
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
        self.publish_profile_state();
        Ok(())
    }

    pub async fn revision(&mut self, profile: Slot) -> Result<u32, Rejection> {
        self.check(profile)?;
        Ok(load(
            &mut self.storage,
            self.order.scope(profile.get()),
            |revision: Revision| revision.0,
        )
        .await
        .unwrap_or_default())
    }

    pub async fn lighting(&mut self, profile: Slot) -> Result<lighting::Settings, Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(self.shared.edit(|appearance| appearance.settings.clone()));
        }
        let limits = self.limits();
        let stored = load_valid(
            &mut self.storage,
            self.order.scope(profile.get()),
            &limits,
            |settings: lighting::Settings| settings,
        )
        .await;
        Ok(stored.unwrap_or_else(|| self.preset(profile.get()).lighting.clone()))
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
            .ok_or(Rejection::StorageInaccessible)?;
        self.unsaved = self.unsaved.union(Changes::LIGHTING);
        Ok(())
    }

    pub async fn key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: &mut Option<KeyColors>,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        let index = usize::from(layer.number());
        if profile.get() == self.active {
            self.shared.edit(|appearance| {
                if let Some(active) = appearance.colors.0.get(index) {
                    colors.clone_from(active);
                }
            });
            return Ok(());
        }
        let key_count = ROWS * COLUMNS;
        let mut fitting = false;
        let found = load_stored::<_, Colors, _>(
            &mut self.storage,
            self.order.scope(profile.get()),
            |stored| {
                decode_sequence(
                    stored,
                    Colors::VERSION,
                    |stored_index, stored_colors: Option<KeyColors>| {
                        if stored_index == index {
                            fitting = fits(stored_colors.as_ref(), key_count);
                            *colors = stored_colors;
                        }
                    },
                )
            },
        )
        .await;
        if found != Some(true) || !fitting {
            *colors = self
                .preset(profile.get())
                .colors
                .get(index)
                .copied()
                .flatten()
                .cloned();
        }
        Ok(())
    }

    pub async fn set_key_colors(
        &mut self,
        profile: Slot,
        layer: Layer,
        colors: Option<&KeyColors>,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            self.shared
                .change(Changes::COLORS, |appearance| {
                    appearance.colors.set(layer, colors.cloned());
                })
                .ok_or(Rejection::StorageInaccessible)?;
            self.unsaved = self.unsaved.union(Changes::COLORS);
            return Ok(());
        }
        let slot = profile.get();
        let preset = self.preset(slot);
        let key_count = ROWS * COLUMNS;
        let mut fitting = [false; LAYERS];
        let mut stored = Colors::default();
        let found = load_stored::<_, Colors, _>(&mut self.storage, self.order.scope(slot), |raw| {
            decode_sequence(
                raw,
                Colors::VERSION,
                |index, layer_colors: Option<KeyColors>| {
                    if let (Some(target), Some(entry)) =
                        (stored.0.get_mut(index), fitting.get_mut(index))
                    {
                        *entry = fits(layer_colors.as_ref(), key_count);
                        *target = layer_colors;
                    }
                },
            )
        })
        .await;
        if found != Some(true) {
            fitting = [false; LAYERS];
        }
        for ((target, factory_colors), kept) in stored.0.iter_mut().zip(preset.colors).zip(fitting)
        {
            if !kept {
                *target = factory_colors.cloned();
            }
        }
        if let Some(target) = stored.0.get_mut(usize::from(layer.number())) {
            *target = colors.cloned();
        }
        self.store_part(slot, &stored).await
    }

    pub async fn lightbar(
        &mut self,
        profile: Slot,
        settings: &mut lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            self.shared
                .edit(|appearance| settings.clone_from(&appearance.lightbar));
            return Ok(());
        }
        let limits = self.limits();
        let found = load_valid(
            &mut self.storage,
            self.order.scope(profile.get()),
            &limits,
            |stored: lightbar::Settings| *settings = stored,
        )
        .await;
        if found.is_none() {
            *settings = self.preset(profile.get()).lightbar();
        }
        Ok(())
    }

    pub async fn set_lightbar(
        &mut self,
        profile: Slot,
        settings: &lightbar::Settings,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            return self.store_part(profile.get(), settings).await;
        }
        self.shared
            .change(Changes::LIGHTBAR, |appearance| {
                appearance.lightbar.clone_from(settings);
            })
            .ok_or(Rejection::StorageInaccessible)?;
        self.unsaved = self.unsaved.union(Changes::LIGHTBAR);
        Ok(())
    }

    #[must_use]
    pub const fn global_lightbar(&self) -> &lightbar::Entries {
        &self.global_lightbar
    }

    pub fn set_global_lightbar(&mut self, entries: &lightbar::Entries) {
        self.global_lightbar.clone_from(entries);
        self.shared.set_global_lightbar(&self.global_lightbar);
        self.unsaved = self.unsaved.union(Changes::GLOBAL_LIGHTBAR);
    }

    pub async fn settings(&mut self, profile: Slot) -> Result<keys::Settings, Rejection> {
        let mut settings = self.preset(profile.get()).keys;
        self.fill(
            profile,
            &mut settings,
            |profile| &profile.settings,
            |stored, settings| *settings = stored,
            |_| {},
        )
        .await?;
        Ok(settings)
    }

    pub async fn set_settings(
        &mut self,
        profile: Slot,
        settings: keys::Settings,
    ) -> Result<(), Rejection> {
        let active = profile.get() == self.active;
        self.set(profile, Parts::SETTINGS, &settings, |profile| {
            &mut profile.settings
        })
        .await?;
        if active {
            self.shared
                .set_fast_scan(settings.scan_mode == ScanMode::Parallel);
        }
        Ok(())
    }

    pub async fn keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &mut Keymap,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        let index = usize::from(layer.number());
        let slot = profile.get();
        if slot == self.active {
            self.running_keymap(index, keymap);
            return Ok(());
        }
        let mut fitting = self
            .stored_keymap(self.order.scope(slot), index, keymap)
            .await;
        if fitting.is_none() && slot != 0 {
            if self.active == 0 {
                self.running_keymap(index, keymap);
                return Ok(());
            }
            fitting = self.stored_keymap(self.order.scope(0), index, keymap).await;
        }
        if fitting != Some(true) {
            self.default_keymap(layer, keymap);
        }
        Ok(())
    }

    pub async fn set_keymap(
        &mut self,
        profile: Slot,
        layer: Layer,
        keymap: &Keymap,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        let index = usize::from(layer.number());
        let slot = profile.get();
        if slot == self.active {
            self.keys
                .change(Parts::KEYMAPS, |profile| {
                    if let Some(target) = profile.keymaps.0.get_mut(index) {
                        target.clone_from(keymap);
                    }
                })
                .ok_or(Rejection::StorageInaccessible)?;
            self.unsaved_keys = self.unsaved_keys.union(Parts::KEYMAPS);
            self.relink = false;
            return Ok(());
        }
        let mut keymaps = Keymaps::default();
        self.keymaps_of(slot, &mut keymaps).await;
        if let Some(target) = keymaps.0.get_mut(index) {
            target.clone_from(keymap);
        }
        self.store_part(slot, &keymaps).await?;
        if slot == 0 {
            self.follow_first_profile().await;
        }
        Ok(())
    }

    pub async fn keymap_link(&mut self, profile: Slot) -> Result<bool, Rejection> {
        self.check(profile)?;
        let slot = profile.get();
        if slot == 0 {
            return Ok(false);
        }
        if slot == self.active {
            if self.relink {
                return Ok(true);
            }
            if self.unsaved_keys.contains(Parts::KEYMAPS) {
                return Ok(false);
            }
        }
        Ok(!self.has_own_keymaps(slot).await)
    }

    pub async fn set_keymap_link(&mut self, profile: Slot, linked: bool) -> Result<(), Rejection> {
        self.check(profile)?;
        let slot = profile.get();
        if slot == 0 {
            return if linked {
                Err(Invalid::FirstProfileLinked.into())
            } else {
                Ok(())
            };
        }
        if self.keymap_link(profile).await? == linked {
            return Ok(());
        }
        if slot == self.active {
            if linked {
                self.relink = true;
                self.unsaved_keys = self.unsaved_keys.without(Parts::KEYMAPS);
                self.reset_keymaps();
                self.load_running_keymaps(self.order.scope(0)).await;
                self.keys.change(Parts::KEYMAPS, |_| ());
            } else {
                self.relink = false;
                self.unsaved_keys = self.unsaved_keys.union(Parts::KEYMAPS);
            }
            return Ok(());
        }
        if linked {
            self.remove_keymaps(slot).await?;
            return self.bump_revision(slot).await;
        }
        let mut keymaps = Keymaps::default();
        self.keymaps_of(slot, &mut keymaps).await;
        self.store_part(slot, &keymaps).await
    }

    pub async fn overrides(
        &mut self,
        profile: Slot,
        overrides: &mut Overrides,
    ) -> Result<(), Rejection> {
        let preset = self.preset(profile.get());
        self.fill(
            profile,
            overrides,
            |profile| &profile.overrides,
            |stored, overrides| *overrides = stored,
            |overrides| fill_overrides(overrides, preset, ROWS * COLUMNS),
        )
        .await
    }

    pub async fn set_overrides(
        &mut self,
        profile: Slot,
        overrides: &Overrides,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::OVERRIDES, overrides, |profile| {
            &mut profile.overrides
        })
        .await
    }

    pub async fn advanced(
        &mut self,
        profile: Slot,
        keys: &mut advanced::Keys,
    ) -> Result<(), Rejection> {
        let preset = self.preset(profile.get());
        self.fill(
            profile,
            keys,
            |profile| &profile.advanced,
            |stored, keys| *keys = stored,
            |keys| fill_advanced(keys, preset),
        )
        .await
    }

    pub async fn set_advanced(
        &mut self,
        profile: Slot,
        keys: &advanced::Keys,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::ADVANCED, keys, |profile| {
            &mut profile.advanced
        })
        .await
    }

    pub async fn gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &mut gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.fill(
            profile,
            mapping,
            |profile| &profile.gamepad_mapping,
            |stored, mapping| *mapping = stored,
            |mapping| clear_mapping(mapping, ROWS * COLUMNS),
        )
        .await
    }

    pub async fn set_gamepad_mapping(
        &mut self,
        profile: Slot,
        mapping: &gamepad::Mapping,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::GAMEPAD_MAPPING, mapping, |profile| {
            &mut profile.gamepad_mapping
        })
        .await
    }

    pub async fn gamepad_settings(
        &mut self,
        profile: Slot,
    ) -> Result<gamepad::Settings, Rejection> {
        let mut settings = default_gamepad_settings();
        self.fill(
            profile,
            &mut settings,
            |profile| &profile.gamepad_settings,
            |stored, settings| *settings = stored,
            |_| {},
        )
        .await?;
        Ok(settings)
    }

    pub async fn set_gamepad_settings(
        &mut self,
        profile: Slot,
        settings: gamepad::Settings,
    ) -> Result<(), Rejection> {
        self.set(profile, Parts::GAMEPAD_SETTINGS, &settings, |profile| {
            &mut profile.gamepad_settings
        })
        .await
    }

    async fn publish(&mut self) {
        let slot = self.active;
        self.shared.begin_activation();
        self.reset_keys(slot);
        let fast_scan = self.load_key_parts(slot).await;
        self.keys.activate(self.count, slot);
        self.reset_appearance(slot);
        self.load_appearance(slot).await;
        self.shared.activate(fast_scan);
        self.info = self.info_of(slot).await;
        self.unsaved_keys = Parts::NONE;
        self.unsaved = self.unsaved.intersection(Changes::DEVICE_WIDE);
        self.relink = false;
        self.publish_profile_state();
    }

    async fn load_key_parts(&mut self, slot: u8) -> bool {
        let scope = self.order.scope(slot);
        let keys = self.keys;
        let limits = self.limits();
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |settings: keys::Settings| {
                keys.edit(|profile| profile.settings = settings);
            },
        )
        .await;
        if !self.load_running_keymaps(scope).await && slot != 0 {
            self.load_running_keymaps(self.order.scope(0)).await;
        }
        load_valid(&mut self.storage, scope, &limits, |overrides: Overrides| {
            keys.edit(|profile| profile.overrides = overrides);
        })
        .await;
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |advanced: advanced::Keys| {
                keys.edit(|profile| profile.advanced = advanced);
            },
        )
        .await;
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |mapping: gamepad::Mapping| {
                keys.edit(|profile| profile.gamepad_mapping = mapping);
            },
        )
        .await;
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |settings: gamepad::Settings| {
                keys.edit(|profile| profile.gamepad_settings = settings);
            },
        )
        .await;
        keys.edit(|profile| profile.settings.scan_mode == ScanMode::Parallel)
    }

    async fn load_running_keymaps(&mut self, scope: Scope) -> bool {
        let keys = self.keys;
        let limits = self.limits();
        load_stored::<_, Keymaps, _>(&mut self.storage, scope, |stored| {
            let mut valid = [false; LAYERS];
            let complete = decode_sequence(stored, Keymaps::VERSION, |index, layer: Keymap| {
                if let Some(entry) = valid.get_mut(index) {
                    *entry = layer.validate(&limits).is_ok();
                }
            });
            if complete {
                decode_sequence(stored, Keymaps::VERSION, |index, layer| {
                    if valid.get(index) == Some(&true) {
                        keys.edit(|profile| {
                            if let Some(target) = profile.keymaps.0.get_mut(index) {
                                *target = layer;
                            }
                        });
                    }
                });
            }
        })
        .await
        .is_some()
    }

    fn running_keymap(&self, index: usize, keymap: &mut Keymap) {
        self.keys.edit(|profile| {
            if let Some(running) = profile.keymaps.0.get(index) {
                keymap.clone_from(running);
            }
        });
    }

    async fn stored_keymap(
        &mut self,
        scope: Scope,
        index: usize,
        keymap: &mut Keymap,
    ) -> Option<bool> {
        let limits = self.limits();
        let mut fitting = false;
        let complete = load_stored::<_, Keymaps, _>(&mut self.storage, scope, |stored| {
            decode_sequence(
                stored,
                Keymaps::VERSION,
                |stored_index, stored_layer: Keymap| {
                    if stored_index == index {
                        fitting = stored_layer.validate(&limits).is_ok();
                        *keymap = stored_layer;
                    }
                },
            )
        })
        .await?;
        Some(complete && fitting)
    }

    async fn keymaps_of(&mut self, slot: u8, keymaps: &mut Keymaps) {
        let mut fitting = [false; LAYERS];
        let own = self
            .stored_keymaps(self.order.scope(slot), keymaps, &mut fitting)
            .await;
        if !own && slot != 0 {
            if self.active == 0 {
                for (index, target) in keymaps.0.iter_mut().enumerate() {
                    self.running_keymap(index, target);
                }
                return;
            }
            self.stored_keymaps(self.order.scope(0), keymaps, &mut fitting)
                .await;
        }
        for ((layer, target), kept) in Layer::ALL
            .into_iter()
            .zip(keymaps.0.iter_mut())
            .zip(fitting)
        {
            if !kept {
                self.default_keymap(layer, target);
            }
        }
    }

    async fn stored_keymaps(
        &mut self,
        scope: Scope,
        keymaps: &mut Keymaps,
        fitting: &mut [bool; LAYERS],
    ) -> bool {
        let limits = self.limits();
        let found = load_stored::<_, Keymaps, _>(&mut self.storage, scope, |stored| {
            decode_sequence(
                stored,
                Keymaps::VERSION,
                |stored_index, stored_layer: Keymap| {
                    if let (Some(target), Some(entry)) = (
                        keymaps.0.get_mut(stored_index),
                        fitting.get_mut(stored_index),
                    ) {
                        *entry = stored_layer.validate(&limits).is_ok();
                        *target = stored_layer;
                    }
                },
            )
        })
        .await;
        if found != Some(true) {
            *fitting = [false; LAYERS];
        }
        found.is_some()
    }

    async fn has_own_keymaps(&mut self, slot: u8) -> bool {
        load_stored::<_, Keymaps, _>(&mut self.storage, self.order.scope(slot), |_| ())
            .await
            .is_some()
    }

    async fn remove_keymaps(&mut self, slot: u8) -> Result<(), Rejection> {
        let key = RecordKey::of::<Keymaps>(self.order.scope(slot));
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::StorageInaccessible);
        };
        storage
            .remove(key)
            .await
            .map_err(|_| Rejection::StorageInaccessible)
    }

    async fn follow_first_profile(&mut self) {
        if self.unsaved_keys.contains(Parts::KEYMAPS)
            || (!self.relink && self.has_own_keymaps(self.active).await)
        {
            return;
        }
        self.reset_keymaps();
        if self.active != 0 {
            self.load_running_keymaps(self.order.scope(0)).await;
        }
        self.keys.change(Parts::KEYMAPS, |_| ());
    }

    async fn load_appearance(&mut self, slot: u8) {
        let scope = self.order.scope(slot);
        let shared = self.shared;
        let key_count = ROWS * COLUMNS;
        let limits = self.limits();
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |settings: lighting::Settings| {
                shared.edit(|appearance| appearance.settings = settings);
            },
        )
        .await;
        load_stored::<_, Colors, _>(&mut self.storage, scope, |stored| {
            let mut fitting = [false; LAYERS];
            let complete = decode_sequence(
                stored,
                Colors::VERSION,
                |index, layer: Option<KeyColors>| {
                    if let Some(entry) = fitting.get_mut(index) {
                        *entry = fits(layer.as_ref(), key_count);
                    }
                },
            );
            if complete {
                decode_sequence(stored, Colors::VERSION, |index, layer| {
                    if fitting.get(index) == Some(&true) {
                        shared.edit(|appearance| {
                            if let Some(target) = appearance.colors.0.get_mut(index) {
                                *target = layer;
                            }
                        });
                    }
                });
            }
        })
        .await;
        load_valid(
            &mut self.storage,
            scope,
            &limits,
            |lightbar: lightbar::Settings| {
                shared.edit(|appearance| appearance.lightbar = lightbar);
            },
        )
        .await;
    }

    fn reset_keys(&self, slot: u8) {
        let preset = self.preset(slot);
        let key_count = ROWS * COLUMNS;
        self.keys.edit(|profile| {
            profile.settings = preset.keys;
            fill_overrides(&mut profile.overrides, preset, key_count);
            fill_advanced(&mut profile.advanced, preset);
            clear_mapping(&mut profile.gamepad_mapping, key_count);
            profile.gamepad_settings = default_gamepad_settings();
        });
        self.reset_keymaps();
    }

    fn reset_keymaps(&self) {
        for (index, layer) in Layer::ALL.into_iter().enumerate() {
            self.keys.edit(|profile| {
                if let Some(keymap) = profile.keymaps.0.get_mut(index) {
                    keymap.clear();
                }
            });
            for action in self.factory.keymap.actions(layer, self.variant) {
                let pushed = self.keys.edit(|profile| {
                    profile
                        .keymaps
                        .0
                        .get_mut(index)
                        .is_some_and(|keymap| keymap.push(action).is_ok())
                });
                if !pushed {
                    break;
                }
            }
        }
    }

    fn reset_appearance(&self, slot: u8) {
        let preset = self.preset(slot);
        self.shared.edit(|appearance| {
            appearance.settings.clone_from(&preset.lighting);
            for (target, colors) in appearance.colors.0.iter_mut().zip(preset.colors) {
                match (target.as_mut(), colors) {
                    (Some(target), Some(colors)) => target.clone_from(colors),
                    (_, colors) => *target = colors.cloned(),
                }
            }
            appearance.lightbar = preset.lightbar();
        });
    }

    fn publish_profile_state(&self) {
        self.keys.set_links(self.selected, self.returning);
        self.shared.set_profile_state(ProfileState {
            slot: self.active,
            count: self.count,
            color: self.info.color,
        });
    }

    async fn load_device_wide(&mut self) {
        self.count = self.factory.created.clamp(1, self.capacity);
        self.global = self.factory.global;
        self.device = device::Settings::from(self.factory.face);
        self.global_lightbar.clone_from(&self.factory.lightbar);
        let capacity = self.capacity;
        let limits = self.limits();
        if let Some(count) = load(&mut self.storage, Scope::Global, |count: Count| {
            count.0.clamp(1, capacity)
        })
        .await
        {
            self.count = count;
        }
        let linked_capacity = self.linked_capacity;
        self.linked = load(&mut self.storage, Scope::Global, |count: LinkedCount| {
            count.0.min(linked_capacity)
        })
        .await
        .unwrap_or(0);
        if self
            .selected
            .is_some_and(|selected| Slot::new(selected).index() >= self.linked)
        {
            self.selected = None;
        }
        self.order = load(&mut self.storage, Scope::Global, |order: Order| {
            order.is_permutation_of(capacity).then_some(order)
        })
        .await
        .flatten()
        .unwrap_or_else(|| Order::identity(capacity));
        if let Some(global) = load_valid(
            &mut self.storage,
            Scope::Global,
            &limits,
            |global: global::Settings| global,
        )
        .await
        {
            self.global = global;
        }
        if let Some(device) = load_valid(
            &mut self.storage,
            Scope::Global,
            &limits,
            |device: device::Settings| device,
        )
        .await
        {
            self.device = device;
        }
        let entries = &mut self.global_lightbar;
        load_valid(
            &mut self.storage,
            Scope::Global,
            &limits,
            |stored: lightbar::Entries| {
                *entries = stored;
            },
        )
        .await;
        self.load_switches(&limits).await;
        self.layout = match self.storage.as_deref_mut() {
            Some(storage) => LayoutOverride::stored(storage).await,
            None => None,
        };
        self.unsaved = Changes::NONE;
        self.apply_global();
        self.shared.set_global_lightbar(&self.global_lightbar);
    }

    async fn load_switches(&mut self, limits: &Capabilities) {
        let keys = self.keys;
        let fitted = load_valid(
            &mut self.storage,
            Scope::Global,
            limits,
            |stored: switches::Settings| keys.set_switches(&stored).is_ok(),
        )
        .await;
        if fitted != Some(true) {
            let mut factory = switches::Settings {
                default: None,
                keys: heapless::Vec::new(),
            };
            let _ = factory.keys.resize(ROWS * COLUMNS, None);
            let _ = keys.set_switches(&factory);
        }
    }

    fn apply_global(&self) {
        self.shared.set_global(self.global);
        self.keys
            .set_rebound_filter(self.global.release_rebound_filter);
    }

    async fn info_of(&mut self, slot: u8) -> Info {
        match load(&mut self.storage, self.order.scope(slot), |info: Info| info).await {
            Some(info) => info,
            None => self.preset(slot).info(),
        }
    }

    async fn fill<T: Record + Validate, P: Clone>(
        &mut self,
        profile: Slot,
        part: &mut P,
        from_active: impl FnOnce(&Profile) -> &P,
        from_stored: impl FnOnce(T, &mut P),
        default: impl FnOnce(&mut P),
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            self.keys
                .edit(|profile| part.clone_from(from_active(profile)));
            return Ok(());
        }
        let limits = self.limits();
        let found = load_valid(
            &mut self.storage,
            self.order.scope(profile.get()),
            &limits,
            |stored: T| from_stored(stored, part),
        )
        .await;
        if found.is_none() {
            default(part);
        }
        Ok(())
    }

    async fn set<T: Record + Clone>(
        &mut self,
        profile: Slot,
        parts: Parts,
        value: &T,
        into_active: impl FnOnce(&mut Profile) -> &mut T,
    ) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() != self.active {
            return self.store_part(profile.get(), value).await;
        }
        self.keys
            .change(parts, |profile| into_active(profile).clone_from(value))
            .ok_or(Rejection::StorageInaccessible)?;
        self.unsaved_keys = self.unsaved_keys.union(parts);
        Ok(())
    }

    pub fn default_keymap(&self, layer: Layer, keymap: &mut Keymap) {
        keymap.clear();
        for action in self.factory.keymap.actions(layer, self.variant) {
            if keymap.push(action).is_err() {
                return;
            }
        }
    }

    async fn store_keys<T: KeyPart>(&mut self) -> Result<(), Rejection> {
        let keys = self.keys;
        store_with::<_, T>(&mut self.storage, self.order.scope(self.active), |body| {
            T::encode(keys, body)
        })
        .await
    }

    async fn store_appearance<T: Record>(
        &mut self,
        part: impl Fn(&Appearance) -> &T,
    ) -> Result<(), Rejection> {
        let shared = self.shared;
        store_with::<_, T>(&mut self.storage, self.order.scope(self.active), |body| {
            shared.edit(|appearance| encode(part(appearance), body))
        })
        .await
    }

    const fn check(&self, profile: Slot) -> Result<(), Rejection> {
        let exists = if profile.is_linked() {
            profile.index() < self.linked
        } else {
            profile.get() < self.count
        };
        if exists {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        }
    }

    fn check_onboard(&self, profile: Slot) -> Result<(), Rejection> {
        if profile.is_linked() {
            return Err(Invalid::ProfileLinked(profile.get()).into());
        }
        self.check(profile)
    }

    fn check_linked(&self, profile: Slot) -> Result<(), Rejection> {
        if !profile.is_linked() {
            return Err(Invalid::ProfileNotLinked(profile.get()).into());
        }
        self.check(profile)
    }

    fn preset(&self, slot: u8) -> &'static Preset {
        if Slot::new(slot).is_linked() {
            self.factory.preset(0)
        } else {
            self.factory.preset(self.order.storage_slot(slot))
        }
    }

    fn limits(&self) -> Capabilities {
        Capabilities {
            key_rows: u8::try_from(ROWS).unwrap_or(u8::MAX),
            key_columns: u8::try_from(COLUMNS).unwrap_or(u8::MAX),
            profiles: self.capacity,
            linked_profiles: self.linked_capacity,
            layouts: heapless::Vec::new(),
            lightbar_leds: u8::MAX,
            host_signals: true,
            switch_models: u8::MAX,
            edition: None,
        }
    }

    async fn store_part<T: Record>(&mut self, slot: u8, value: &T) -> Result<(), Rejection> {
        store(&mut self.storage, self.order.scope(slot), value).await?;
        self.bump_revision(slot).await
    }

    async fn bump_revision(&mut self, slot: u8) -> Result<(), Rejection> {
        let scope = self.order.scope(slot);
        let revision = load(&mut self.storage, scope, |revision: Revision| revision.0)
            .await
            .unwrap_or_default();
        store(
            &mut self.storage,
            scope,
            &Revision(revision.wrapping_add(1)),
        )
        .await
    }

    async fn reorder(&mut self, order: Order) -> Result<(), Rejection> {
        if order == self.order {
            return Ok(());
        }
        store(&mut self.storage, Scope::Global, &order).await?;
        let active = self.order.storage_slot(self.active);
        let returning = self.order.storage_slot(self.returning);
        let first = self.order.storage_slot(0);
        self.order = order;
        self.active = self.order.slot_of(active).unwrap_or(self.active);
        self.returning = self.order.slot_of(returning).unwrap_or(self.returning);
        self.keys.set_profiles(self.count, self.active);
        self.publish_profile_state();
        if self.order.storage_slot(0) != first {
            self.follow_first_profile().await;
        }
        Ok(())
    }

    async fn remove_profile(&mut self, slot: u8) -> Result<(), Rejection> {
        let scope = self.order.scope(slot);
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::StorageInaccessible);
        };
        let parts = [
            RecordKey::of::<keys::Settings>(scope),
            RecordKey::of::<Keymaps>(scope),
            RecordKey::of::<Overrides>(scope),
            RecordKey::of::<advanced::Keys>(scope),
            RecordKey::of::<gamepad::Mapping>(scope),
            RecordKey::of::<gamepad::Settings>(scope),
            RecordKey::of::<lighting::Settings>(scope),
            RecordKey::of::<Colors>(scope),
            RecordKey::of::<Info>(scope),
            RecordKey::of::<Revision>(scope),
            RecordKey::of::<lightbar::Settings>(scope),
        ];
        let links = (0..apps::MAX_LINKS)
            .filter(|_| Slot::new(slot).is_linked())
            .map(|index| app_link_key(scope, index));
        for key in parts.into_iter().chain(links) {
            storage
                .remove(key)
                .await
                .map_err(|_| Rejection::StorageInaccessible)?;
        }
        Ok(())
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

async fn load<F: MultiwriteNorFlash, T: Record, R>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    read: impl FnOnce(T) -> R,
) -> Option<R> {
    storage
        .as_mut()?
        .load_with(RecordKey::of::<T>(scope), |stored| {
            stored.decode::<T>().map(read)
        })
        .await
        .ok()
        .flatten()
        .flatten()
}

async fn load_valid<F: MultiwriteNorFlash, T: Record + Validate, R>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    limits: &Capabilities,
    read: impl FnOnce(T) -> R,
) -> Option<R> {
    load(storage, scope, |stored: T| {
        stored.validate(limits).is_ok().then(|| read(stored))
    })
    .await
    .flatten()
}

async fn store<F: MultiwriteNorFlash, T: Record>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    value: &T,
) -> Result<(), Rejection> {
    store_with::<F, T>(storage, scope, |body| encode(value, body)).await
}

async fn store_with<F: MultiwriteNorFlash, T: Record>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    encode: impl Fn(&mut [u8]) -> Option<usize>,
) -> Result<(), Rejection> {
    let Some(storage) = storage.as_mut() else {
        return Err(Rejection::StorageInaccessible);
    };
    storage
        .store_with(RecordKey::of::<T>(scope), T::VERSION, encode)
        .await
        .map_err(|_| Rejection::StorageInaccessible)
}

fn encode(value: &impl Serialize, body: &mut [u8]) -> Option<usize> {
    postcard::to_slice(value, body)
        .ok()
        .map(|encoded| encoded.len())
}

fn encode_colors<M: RawMutex>(shared: &Shared<M>, buffer: &mut [u8]) -> Option<usize> {
    let mut length = 0;
    for layer in 0..LAYERS {
        let count = shared.read(|appearance| {
            appearance
                .colors
                .0
                .get(layer)
                .map(|colors| colors.as_ref().map(|entries| entries.len()))
        })??;
        length = push_encoded(buffer, length, &count.is_some())?;
        let Some(count) = count else {
            continue;
        };
        length = push_encoded(buffer, length, &count)?;
        for index in 0..count {
            let color = shared.read(|appearance| {
                appearance
                    .colors
                    .0
                    .get(layer)?
                    .as_ref()?
                    .get(index)
                    .copied()
            })??;
            length = push_encoded(buffer, length, &color)?;
        }
    }
    Some(length)
}

fn encode_switches<M: RawMutex>(keys: &crate::keys::Shared<M>, buffer: &mut [u8]) -> Option<usize> {
    let mut length = push_encoded(buffer, 0, &keys.switches(|settings| settings.default))?;
    let count = keys.switches(|settings| settings.keys.len());
    length = push_encoded(buffer, length, &count)?;
    for index in 0..count {
        let switch = keys.switches(|settings| settings.keys.get(index).copied())?;
        length = push_encoded(buffer, length, &switch)?;
    }
    Some(length)
}

fn push_encoded(buffer: &mut [u8], length: usize, value: &impl Serialize) -> Option<usize> {
    let written = postcard::to_slice(value, buffer.get_mut(length..)?)
        .ok()?
        .len();
    length.checked_add(written)
}

async fn load_stored<F: MultiwriteNorFlash, T: Record, R>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    read: impl FnOnce(Stored<'_>) -> R,
) -> Option<R> {
    load_key(storage, RecordKey::of::<T>(scope), read).await
}

const fn app_link_key(scope: Scope, index: u8) -> RecordKey {
    RecordKey::new(scope, APP_LINK_ITEMS + index)
}

async fn load_key<F: MultiwriteNorFlash, R>(
    storage: &mut Option<&mut Settings<F>>,
    key: RecordKey,
    read: impl FnOnce(Stored<'_>) -> R,
) -> Option<R> {
    storage.as_mut()?.load_with(key, read).await.ok().flatten()
}

async fn store_app_link<F: MultiwriteNorFlash>(
    storage: &mut Option<&mut Settings<F>>,
    key: RecordKey,
    link: &apps::Link,
) -> Result<(), Rejection> {
    let Some(storage) = storage.as_mut() else {
        return Err(Rejection::StorageInaccessible);
    };
    storage
        .store_with(key, APP_LINK_VERSION, |body| encode(link, body))
        .await
        .map_err(|_| Rejection::StorageInaccessible)
}

fn decode_sequence<T: DeserializeOwned>(
    stored: Stored<'_>,
    version: u8,
    mut item: impl FnMut(usize, T),
) -> bool {
    if stored.version != version {
        return false;
    }
    let mut rest = stored.body;
    for index in 0..LAYERS {
        let Ok((value, remaining)) = postcard::take_from_bytes::<T>(rest) else {
            return false;
        };
        item(index, value);
        rest = remaining;
    }
    true
}

fn fill_overrides(overrides: &mut Overrides, preset: &Preset, key_count: usize) {
    overrides.clear();
    overrides.extend(core::iter::repeat_n(Override::default(), key_count));
    for (key, actuation) in preset.actuation_points {
        if let Some(entry) = overrides.get_mut(usize::from(key.get())) {
            entry.actuation = Some(*actuation);
        }
    }
}

fn fill_advanced(keys: &mut advanced::Keys, preset: &Preset) {
    keys.clear();
    for key in preset.advanced {
        if keys.push(*key).is_err() {
            break;
        }
    }
}

fn clear_mapping(mapping: &mut gamepad::Mapping, key_count: usize) {
    mapping.bindings.clear();
    mapping
        .bindings
        .extend(core::iter::repeat_n(None, key_count));
    mapping.exclusive = false;
}

fn fits(colors: Option<&KeyColors>, key_count: usize) -> bool {
    colors.is_none_or(|colors| colors.len() == key_count)
}

#[cfg(test)]
mod tests {
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;
    use openwoot_protocol::{heapless::Vec, keys::Action, lighting::Rgb};

    use super::*;

    #[test]
    fn keymap_layers_decode_one_by_one_like_the_whole_record() {
        let mut keymaps = Keymaps::default();
        for (index, keymap) in keymaps.0.iter_mut().enumerate() {
            keymap
                .extend((0..3).map(|key| Action::Keyboard(u8::try_from(index * 3 + key).unwrap())));
        }
        let mut body = [0; 64];
        let len = encode(&keymaps, &mut body).unwrap();
        let stored = Stored {
            version: Keymaps::VERSION,
            body: &body[..len],
        };
        let mut layers = Keymaps::default();
        assert!(decode_sequence(
            stored,
            Keymaps::VERSION,
            |index, keymap| layers.0[index] = keymap
        ));
        assert_eq!(layers, keymaps);
        assert_eq!(stored.decode::<Keymaps>(), Some(keymaps));
        let truncated = Stored {
            version: Keymaps::VERSION,
            body: &body[..len - 1],
        };
        assert!(!decode_sequence(
            truncated,
            Keymaps::VERSION,
            |_, _: Keymap| {}
        ));
        let newer = Stored {
            version: Keymaps::VERSION + 1,
            ..stored
        };
        assert!(!decode_sequence(newer, Keymaps::VERSION, |_, _: Keymap| {}));
    }

    #[test]
    fn colours_encoded_entry_by_entry_equal_the_whole_record() {
        let colors = Colors([
            Some(Vec::from_array(
                [Rgb {
                    red: 1,
                    green: 2,
                    blue: 3,
                }; 3],
            )),
            None,
            Some(KeyColors::new()),
            Some(Vec::from_array(
                [Rgb {
                    red: 200,
                    green: 0,
                    blue: 255,
                }; MAX_KEYS],
            )),
        ]);
        let shared = Shared::<NoopRawMutex>::new();
        shared.edit(|appearance| appearance.colors = colors.clone());
        let mut whole = [0; 2048];
        let whole_len = encode(&colors, &mut whole).unwrap();
        let mut entries = [0; 2048];
        let entries_len = encode_colors(&shared, &mut entries).unwrap();
        assert_eq!(entries[..entries_len], whole[..whole_len]);
        assert_eq!(encode_colors(&shared, &mut entries[..whole_len - 1]), None);
        assert_eq!(
            encode_colors(&Shared::<NoopRawMutex>::new(), &mut entries),
            None
        );
    }

    #[test]
    fn switches_encoded_entry_by_entry_equal_the_whole_record() {
        let chosen = |model, travel_offset| {
            Some(switches::Switch {
                model,
                travel_offset,
            })
        };
        let mut settings = switches::Settings {
            default: chosen(3, 0),
            keys: Vec::from_array([None; MAX_KEYS]),
        };
        settings.keys[0] = chosen(1, 20);
        settings.keys[MAX_KEYS - 1] = chosen(48, 1);
        let keys = crate::keys::Shared::<NoopRawMutex>::new();
        keys.set_switches(&settings).unwrap();
        let mut whole = [0; 2048];
        let whole_len = encode(&settings, &mut whole).unwrap();
        let mut entries = [0; 2048];
        let entries_len = encode_switches(&keys, &mut entries).unwrap();
        assert_eq!(entries[..entries_len], whole[..whole_len]);
        assert_eq!(encode_switches(&keys, &mut entries[..whole_len - 1]), None);
    }
}
