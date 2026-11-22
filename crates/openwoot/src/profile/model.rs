use core::{
    future::Future,
    pin::{Pin, pin},
};

use embassy_sync::blocking_mutex::raw::RawMutex;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::{
    advanced,
    configuration::Rejection,
    device, gamepad, global,
    keys::{self, Keymap, Layer, Override, Overrides, ScanMode},
    lightbar,
    lighting::{self, KeyColors},
    profile::{Info, Slot, Summary},
};
use serde::{Serialize, de::DeserializeOwned};

use super::{Appearance, Changes, Colors, Count, Factory, KeyPart, Preset, Revision, Shared};
use crate::{
    keys::{ActiveProfile, Keymaps, Parts, Profile, default_gamepad_settings},
    layout::LAYERS,
    lighting::ProfileState,
    storage::{Record, RecordKey, Scope, Settings, Stored},
};

pub struct Model<'a, M: RawMutex, F: MultiwriteNorFlash, const ROWS: usize, const COLUMNS: usize> {
    keys: &'a crate::keys::Shared<M>,
    shared: &'a Shared<M>,
    storage: Option<&'a mut Settings<F>>,
    factory: &'static Factory<ROWS, COLUMNS>,
    variant: u8,
    capacity: u8,
    count: u8,
    active: u8,
    info: Info,
    global: global::Settings,
    global_lightbar: lightbar::Entries,
    device: device::Settings,
    unsaved_keys: Parts,
    unsaved: Changes,
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
        let capacity = capacity.max(1);
        Self {
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
            global_lightbar: lightbar::Entries::new(),
            device: device::Settings::from(factory.face),
            unsaved_keys: Parts::NONE,
            unsaved: Changes::NONE,
        }
    }

    pub async fn load(&mut self) {
        self.load_device_wide().await;
        let stored = load(&mut self.storage, Scope::Global, |active: ActiveProfile| {
            active.0
        })
        .await
        .unwrap_or(0);
        self.active = if stored < self.count { stored } else { 0 };
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
    pub const fn summary(&self) -> Summary {
        Summary {
            count: self.count,
            active: Slot::new(self.active),
        }
    }

    pub async fn activate(&mut self, profile: Slot) -> Result<(), Rejection> {
        self.check(profile)?;
        if profile.get() == self.active {
            return Ok(());
        }
        let saved = self.save().await;
        self.active = profile.get();
        self.publish().await;
        let stored = store(
            &mut self.storage,
            Scope::Global,
            &ActiveProfile(self.active),
        )
        .await;
        saved.and(stored)
    }

    pub async fn create(&mut self) -> Result<Slot, Rejection> {
        if self.count >= self.capacity {
            return Err(Rejection::ProfileLimitReached);
        }
        let slot = self.count;
        self.remove_profile(slot).await?;
        self.count += 1;
        store(&mut self.storage, Scope::Global, &Count(self.count)).await?;
        self.keys.set_profiles(self.count, self.active);
        self.publish_profile_state();
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
        store(&mut self.storage, Scope::Global, &Count(self.count)).await?;
        if self.active >= self.count {
            self.activate(Slot::new(self.count - 1)).await
        } else {
            self.keys.set_profiles(self.count, self.active);
            self.publish_profile_state();
            Ok(())
        }
    }

    pub async fn save(&mut self) -> Result<(), Rejection> {
        let keys = self.unsaved_keys;
        let unsaved = self.unsaved;
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
            self.store_appearance(|appearance| &appearance.colors)
                .await?;
        }
        if unsaved.contains(Changes::LIGHTBAR) {
            self.store_appearance(|appearance| &appearance.lightbar)
                .await?;
        }
        if unsaved.contains(Changes::INFO) {
            store(&mut self.storage, Scope::Profile(self.active), &self.info).await?;
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
        if !keys.is_empty() || !unsaved.without(Changes::DEVICE_WIDE).is_empty() {
            self.bump_revision(self.active).await?;
        }
        self.unsaved_keys = Parts::NONE;
        self.unsaved = Changes::NONE;
        Ok(())
    }

    pub async fn revert(&mut self) {
        self.load_device_wide().await;
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
        self.load_device_wide().await;
        self.publish().await;
        Ok(())
    }

    pub async fn save_brightness(&mut self) -> Result<(), Rejection> {
        self.store_appearance(|appearance| &appearance.settings)
            .await?;
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
        self.publish_profile_state();
        Ok(())
    }

    pub async fn revision(&mut self, profile: Slot) -> Result<u32, Rejection> {
        self.check(profile)?;
        Ok(load(
            &mut self.storage,
            Scope::Profile(profile.get()),
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
        let stored = load(
            &mut self.storage,
            Scope::Profile(profile.get()),
            |settings: lighting::Settings| settings,
        )
        .await;
        Ok(stored.unwrap_or_else(|| self.factory.preset(profile.get()).lighting.clone()))
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
        let found = load_stored::<_, Colors, _>(
            &mut self.storage,
            Scope::Profile(profile.get()),
            |stored| {
                decode_sequence(stored, Colors::VERSION, |stored_index, stored_colors| {
                    if stored_index == index {
                        *colors = stored_colors;
                    }
                })
            },
        )
        .await;
        if found != Some(true) {
            *colors = self
                .factory
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
        let preset = self.factory.preset(slot);
        let mut stored = Colors::default();
        let found = load_stored::<_, Colors, _>(&mut self.storage, Scope::Profile(slot), |raw| {
            decode_sequence(raw, Colors::VERSION, |index, layer_colors| {
                if let Some(target) = stored.0.get_mut(index) {
                    *target = layer_colors;
                }
            })
        })
        .await;
        if found != Some(true) {
            for (target, colors) in stored.0.iter_mut().zip(preset.colors) {
                *target = colors.cloned();
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
        let found = load(
            &mut self.storage,
            Scope::Profile(profile.get()),
            |stored: lightbar::Settings| *settings = stored,
        )
        .await;
        if found.is_none() {
            *settings = self.factory.preset(profile.get()).lightbar();
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
        let mut settings = self.factory.preset(profile.get()).keys;
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
        if profile.get() == self.active {
            self.keys.edit(|profile| {
                if let Some(active) = profile.keymaps.0.get(index) {
                    keymap.clone_from(active);
                }
            });
            return Ok(());
        }
        let found = load_stored::<_, Keymaps, _>(
            &mut self.storage,
            Scope::Profile(profile.get()),
            |stored| {
                decode_sequence(stored, Keymaps::VERSION, |stored_index, stored_layer| {
                    if stored_index == index {
                        *keymap = stored_layer;
                    }
                })
            },
        )
        .await;
        if found != Some(true) {
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
        if profile.get() == self.active {
            self.keys
                .change(Parts::KEYMAPS, |profile| {
                    if let Some(target) = profile.keymaps.0.get_mut(index) {
                        target.clone_from(keymap);
                    }
                })
                .ok_or(Rejection::StorageInaccessible)?;
            self.unsaved_keys = self.unsaved_keys.union(Parts::KEYMAPS);
            return Ok(());
        }
        let slot = profile.get();
        let mut keymaps = Keymaps::default();
        let found =
            load_stored::<_, Keymaps, _>(&mut self.storage, Scope::Profile(slot), |stored| {
                decode_sequence(stored, Keymaps::VERSION, |stored_index, stored_layer| {
                    if let Some(target) = keymaps.0.get_mut(stored_index) {
                        *target = stored_layer;
                    }
                })
            })
            .await;
        if found != Some(true) {
            for (layer, target) in Layer::ALL.into_iter().zip(keymaps.0.iter_mut()) {
                self.default_keymap(layer, target);
            }
        }
        if let Some(target) = keymaps.0.get_mut(index) {
            target.clone_from(keymap);
        }
        self.store_part(slot, &keymaps).await
    }

    pub async fn overrides(
        &mut self,
        profile: Slot,
        overrides: &mut Overrides,
    ) -> Result<(), Rejection> {
        let preset = self.factory.preset(profile.get());
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
        let preset = self.factory.preset(profile.get());
        self.fill(
            profile,
            keys,
            |profile| &profile.advanced,
            |stored, keys| *keys = stored,
            |keys| {
                keys.clear();
                keys.extend(preset.advanced.iter().copied());
            },
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
            |mapping| {
                mapping.bindings.clear();
                mapping
                    .bindings
                    .extend(core::iter::repeat_n(None, ROWS * COLUMNS));
                mapping.exclusive = false;
            },
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
        let scope = Scope::Profile(slot);
        let (keys, shared) = (self.keys, self.shared);
        let key_count = ROWS * COLUMNS;
        self.reset_keys(slot);
        load(&mut self.storage, scope, |settings: keys::Settings| {
            keys.edit(|profile| profile.settings = settings);
        })
        .await;
        load_stored::<_, Keymaps, _>(&mut self.storage, scope, |stored| {
            let mut valid = true;
            let complete = decode_sequence(stored, Keymaps::VERSION, |_, layer: Keymap| {
                valid &= layer.len() == key_count;
            });
            if complete && valid {
                decode_sequence(stored, Keymaps::VERSION, |index, layer| {
                    keys.edit(|profile| {
                        if let Some(target) = profile.keymaps.0.get_mut(index) {
                            *target = layer;
                        }
                    });
                });
            }
        })
        .await;
        load(&mut self.storage, scope, |overrides: Overrides| {
            if overrides.len() == key_count {
                keys.edit(|profile| profile.overrides = overrides);
            }
        })
        .await;
        load(&mut self.storage, scope, |advanced: advanced::Keys| {
            keys.edit(|profile| profile.advanced = advanced);
        })
        .await;
        load(&mut self.storage, scope, |mapping: gamepad::Mapping| {
            if mapping.bindings.len() == key_count {
                keys.edit(|profile| profile.gamepad_mapping = mapping);
            }
        })
        .await;
        load(&mut self.storage, scope, |settings: gamepad::Settings| {
            keys.edit(|profile| profile.gamepad_settings = settings);
        })
        .await;
        let fast_scan = keys.edit(|profile| profile.settings.scan_mode == ScanMode::Parallel);
        keys.activate(self.count, slot);
        self.reset_appearance(slot);
        load(&mut self.storage, scope, |settings: lighting::Settings| {
            shared.edit(|appearance| appearance.settings = settings);
        })
        .await;
        load_stored::<_, Colors, _>(&mut self.storage, scope, |stored| {
            if decode_sequence(stored, Colors::VERSION, |_, _: Option<KeyColors>| {}) {
                decode_sequence(stored, Colors::VERSION, |index, layer| {
                    shared.edit(|appearance| {
                        if let Some(target) = appearance.colors.0.get_mut(index) {
                            *target = layer;
                        }
                    });
                });
            }
        })
        .await;
        load(&mut self.storage, scope, |lightbar: lightbar::Settings| {
            shared.edit(|appearance| appearance.lightbar = lightbar);
        })
        .await;
        shared.activate(fast_scan);
        self.info = self.info_of(slot).await;
        self.unsaved_keys = Parts::NONE;
        self.unsaved = self.unsaved.intersection(Changes::DEVICE_WIDE);
        self.publish_profile_state();
    }

    fn reset_keys(&self, slot: u8) {
        let preset = self.factory.preset(slot);
        let key_count = ROWS * COLUMNS;
        self.keys.edit(|profile| {
            profile.settings = preset.keys;
            fill_overrides(&mut profile.overrides, preset, key_count);
            profile.advanced.clear();
            profile.advanced.extend(preset.advanced.iter().copied());
            profile.gamepad_mapping.bindings.clear();
            profile
                .gamepad_mapping
                .bindings
                .extend(core::iter::repeat_n(None, key_count));
            profile.gamepad_mapping.exclusive = false;
            profile.gamepad_settings = default_gamepad_settings();
        });
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
        let preset = self.factory.preset(slot);
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
        if let Some(count) = load(&mut self.storage, Scope::Global, |count: Count| {
            count.0.clamp(1, capacity)
        })
        .await
        {
            self.count = count;
        }
        if let Some(global) = load(&mut self.storage, Scope::Global, |global| global).await {
            self.global = global;
        }
        if let Some(device) = load(&mut self.storage, Scope::Global, |device| device).await {
            self.device = device;
        }
        let entries = &mut self.global_lightbar;
        load(
            &mut self.storage,
            Scope::Global,
            |stored: lightbar::Entries| {
                *entries = stored;
            },
        )
        .await;
        self.unsaved = Changes::NONE;
        self.apply_global();
        self.shared.set_global_lightbar(&self.global_lightbar);
    }

    fn apply_global(&self) {
        self.shared.set_global(self.global);
        self.keys
            .set_rebound_filter(self.global.release_rebound_filter);
    }

    async fn info_of(&mut self, slot: u8) -> Info {
        match load(&mut self.storage, Scope::Profile(slot), |info: Info| info).await {
            Some(info) => info,
            None => self.factory.preset(slot).info(),
        }
    }

    async fn fill<T: Record, P: Clone>(
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
        let found = load(
            &mut self.storage,
            Scope::Profile(profile.get()),
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

    fn default_keymap(&self, layer: Layer, keymap: &mut Keymap) {
        keymap.clear();
        for action in self.factory.keymap.actions(layer, self.variant) {
            if keymap.push(action).is_err() {
                return;
            }
        }
    }

    async fn store_keys<T: KeyPart>(&mut self) -> Result<(), Rejection> {
        let keys = self.keys;
        store_with::<_, T>(&mut self.storage, Scope::Profile(self.active), |body| {
            T::encode(keys, body)
        })
        .await
    }

    async fn store_appearance<T: Record>(
        &mut self,
        part: impl Fn(&Appearance) -> &T,
    ) -> Result<(), Rejection> {
        let shared = self.shared;
        store_with::<_, T>(&mut self.storage, Scope::Profile(self.active), |body| {
            shared.edit(|appearance| encode(part(appearance), body))
        })
        .await
    }

    const fn check(&self, profile: Slot) -> Result<(), Rejection> {
        if profile.get() < self.count {
            Ok(())
        } else {
            Err(Rejection::ProfileMissing(profile.get()))
        }
    }

    async fn store_part<T: Record>(&mut self, slot: u8, value: &T) -> Result<(), Rejection> {
        store(&mut self.storage, Scope::Profile(slot), value).await?;
        self.bump_revision(slot).await
    }

    async fn bump_revision(&mut self, slot: u8) -> Result<(), Rejection> {
        let scope = Scope::Profile(slot);
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

    async fn remove_profile(&mut self, slot: u8) -> Result<(), Rejection> {
        let Some(storage) = self.storage.as_mut() else {
            return Err(Rejection::StorageInaccessible);
        };
        let scope = Scope::Profile(slot);
        for key in [
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
        ] {
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

async fn load_stored<F: MultiwriteNorFlash, T: Record, R>(
    storage: &mut Option<&mut Settings<F>>,
    scope: Scope,
    read: impl FnOnce(Stored<'_>) -> R,
) -> Option<R> {
    storage
        .as_mut()?
        .load_with(RecordKey::of::<T>(scope), read)
        .await
        .ok()
        .flatten()
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

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Action;

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
}
