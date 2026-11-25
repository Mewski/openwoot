use gpui_kit::{
    App, AppContext as _, Context, Entity, Global, SharedString, Subscription, Window,
    component::{WindowExt as _, notification::Notification},
};
use openwoot_protocol::{
    configuration::{Invalid, Validate as _},
    endpoints::{GetKeyOverrides, GetKeySettings, SetKeyOverrides, SetKeySettings},
    keys::{self, Distance, RapidTrigger, ScanMode},
    profile::{Change, Slot},
};

use super::{
    edits,
    effective::{self, Selected},
    ranges::{RELEASE_DEFAULT_MM, Span},
};
use crate::{
    keyboard::Selection,
    store::{Keyboard, Scope, Store, StoreEvent},
};

pub(crate) struct Tuning {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<keys::Settings>,
    overrides: Option<keys::Overrides>,
    revision: u64,
    selection: Selection,
    _subscription: Subscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Press,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Toggle {
    Off,
    On,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reading {
    pub(crate) millimetres: f32,
    pub(crate) mixed: bool,
    pub(crate) span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Current {
    pub(crate) actuation: Reading,
    pub(crate) rapid_trigger: Toggle,
    pub(crate) split: Toggle,
    pub(crate) continuous: Toggle,
    pub(crate) press: Reading,
    pub(crate) release: Reading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyTuning {
    pub(crate) actuation: Distance,
    pub(crate) rapid_trigger: Option<RapidTrigger>,
}

struct Shared(Entity<Tuning>);

impl Global for Shared {}

impl Tuning {
    pub(crate) fn shared(store: &Entity<Store>, cx: &mut App) -> Entity<Self> {
        if let Some(shared) = cx.try_global::<Shared>() {
            return shared.0.clone();
        }
        let store = store.clone();
        let tuning = cx.new(|cx| Self::new(store, cx));
        cx.set_global(Shared(tuning.clone()));
        tuning
    }

    fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe(&store, |this, _, event, cx| match event {
            StoreEvent::Connected => {
                this.selection.clear();
                this.load(cx);
            }
            StoreEvent::ProfileChanged | StoreEvent::Reloaded => this.load(cx),
            StoreEvent::Disconnected | StoreEvent::Failed(_) => {}
        });
        let mut tuning = Self {
            store,
            profile: None,
            settings: None,
            overrides: None,
            revision: 0,
            selection: Selection::default(),
            _subscription: subscription,
        };
        tuning.load(cx);
        tuning
    }

    pub(crate) const fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(crate) const fn is_loaded(&self) -> bool {
        self.settings.is_some() && self.overrides.is_some()
    }

    pub(crate) const fn profile(&self) -> Option<Slot> {
        self.profile
    }

    pub(crate) fn tachyon(&self) -> Option<bool> {
        self.settings
            .map(|settings| settings.scan_mode == ScanMode::Parallel)
    }

    pub(crate) fn key(&self, key: usize) -> Option<KeyTuning> {
        let settings = self.settings.as_ref()?;
        let entry = self
            .overrides
            .as_ref()?
            .get(key)
            .copied()
            .unwrap_or_default();
        Some(KeyTuning {
            actuation: effective::actuation(settings, &entry),
            rapid_trigger: effective::rapid_trigger(settings, &entry),
        })
    }

    pub(crate) fn rapid_trigger_keys(&self, keyboard: &Keyboard) -> usize {
        present(keyboard)
            .filter(|key| {
                self.key(*key)
                    .is_some_and(|tuning| tuning.rapid_trigger.is_some())
            })
            .count()
    }

    pub(crate) fn current(&self, keyboard: &Keyboard) -> Option<Current> {
        let settings = self.settings.as_ref()?;
        let overrides = self.overrides.as_ref()?;
        let keys: Vec<usize> = if self.selection.is_empty() {
            present(keyboard).collect()
        } else {
            self.selection.iter().collect()
        };
        let travel = keys
            .iter()
            .map(|key| keyboard.travel_mm(*key))
            .reduce(f32::min)?;
        let entries: Vec<(usize, keys::Override)> = keys
            .iter()
            .map(|key| (*key, overrides.get(*key).copied().unwrap_or_default()))
            .collect();
        let parameters: Vec<(usize, RapidTrigger)> = entries
            .iter()
            .map(|(key, entry)| (*key, effective::parameters(settings, entry)))
            .collect();
        let sensitivity = Span::sensitivity(travel);
        Some(Current {
            actuation: reading(
                entries.iter().map(|(key, entry)| {
                    keyboard.distance_mm(*key, effective::actuation(settings, entry))
                }),
                Span::actuation(travel),
            ),
            rapid_trigger: toggle(
                entries
                    .iter()
                    .map(|(_, entry)| effective::rapid_trigger(settings, entry).is_some()),
            ),
            split: toggle(
                parameters
                    .iter()
                    .map(|(_, parameters)| parameters.release.is_some()),
            ),
            continuous: toggle(
                parameters
                    .iter()
                    .map(|(_, parameters)| parameters.continuous),
            ),
            press: reading(
                parameters
                    .iter()
                    .map(|(key, parameters)| keyboard.distance_mm(*key, parameters.press)),
                sensitivity,
            ),
            release: reading(
                parameters.iter().map(|(key, parameters)| {
                    keyboard.distance_mm(*key, parameters.release.unwrap_or(parameters.press))
                }),
                sensitivity,
            ),
        })
    }

    pub(crate) fn toggle_key(&mut self, key: usize, cx: &mut Context<Self>) {
        self.selection.toggle(key);
        cx.notify();
    }

    pub(crate) fn sweep_key(&mut self, key: usize, cx: &mut Context<Self>) {
        if !self.selection.contains(key) {
            self.selection.insert(key);
            cx.notify();
        }
    }

    pub(crate) fn select_all(&mut self, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            self.selection.select_all(keyboard);
        }
        cx.notify();
    }

    pub(crate) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selection.clear();
        cx.notify();
    }

    pub(crate) fn set_actuation(
        &mut self,
        millimetres: f32,
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        self.change_overrides(
            |settings, overrides, keys, keyboard| {
                for key in keys {
                    let actuation = keyboard.distance_from_mm(*key, millimetres);
                    edits::set_actuation(settings, overrides, *key, actuation);
                }
            },
            cx,
        )
    }

    pub(crate) fn set_rapid_trigger(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        self.change_overrides(
            |settings, overrides, keys, _| {
                for key in keys {
                    let entry = overrides.get(*key).copied().unwrap_or_default();
                    if effective::rapid_trigger(settings, &entry).is_some() != enabled {
                        let rapid_trigger =
                            enabled.then(|| effective::parameters(settings, &entry));
                        edits::set_rapid_trigger(settings, overrides, *key, rapid_trigger);
                    }
                }
            },
            cx,
        )
    }

    pub(crate) fn set_sensitivity(
        &mut self,
        side: Side,
        millimetres: f32,
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        self.change_parameters(
            |parameters, key, keyboard| {
                let distance = keyboard.distance_from_mm(key, millimetres);
                match side {
                    Side::Press => parameters.press = distance,
                    Side::Release => parameters.release = Some(distance),
                }
            },
            cx,
        )
    }

    pub(crate) fn set_split(&mut self, split: bool, cx: &mut Context<Self>) -> Result<(), Invalid> {
        self.change_parameters(
            |parameters, key, keyboard| {
                parameters.release = split.then(|| {
                    parameters.release.unwrap_or_else(|| {
                        let release =
                            Span::sensitivity(keyboard.travel_mm(key)).snap(RELEASE_DEFAULT_MM);
                        keyboard.distance_from_mm(key, release)
                    })
                });
            },
            cx,
        )
    }

    pub(crate) fn set_continuous(
        &mut self,
        continuous: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        self.change_parameters(|parameters, _, _| parameters.continuous = continuous, cx)
    }

    pub(crate) fn set_tachyon(
        &mut self,
        tachyon: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        let Some(profile) = self.editable_profile(cx) else {
            self.load(cx);
            return Ok(());
        };
        let Some(before) = self.settings else {
            return Ok(());
        };
        let mut settings = before;
        settings.scan_mode = if tachyon {
            ScanMode::Parallel
        } else {
            ScanMode::Single
        };
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            settings.validate(&keyboard.capabilities)?;
        }
        if settings == before {
            return Ok(());
        }
        self.settings = Some(settings);
        self.revision = self.revision.wrapping_add(1);
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetKeySettings>(
                &Change {
                    profile,
                    value: before,
                },
                Change {
                    profile,
                    value: settings,
                },
                Scope::Profile(profile),
                cx,
            )
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_err() {
                this.update(cx, Self::load).ok();
            }
        })
        .detach();
        cx.notify();
        Ok(())
    }

    fn change_parameters(
        &mut self,
        edit: impl Fn(&mut RapidTrigger, usize, &Keyboard),
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        self.change_overrides(
            |settings, overrides, keys, keyboard| {
                for key in keys {
                    let entry = overrides.get(*key).copied().unwrap_or_default();
                    if let Some(mut parameters) = effective::rapid_trigger(settings, &entry) {
                        edit(&mut parameters, *key, keyboard);
                        edits::set_rapid_trigger(settings, overrides, *key, Some(parameters));
                    }
                }
            },
            cx,
        )
    }

    fn change_overrides(
        &mut self,
        edit: impl FnOnce(&keys::Settings, &mut keys::Overrides, &[usize], &Keyboard),
        cx: &mut Context<Self>,
    ) -> Result<(), Invalid> {
        let Some(profile) = self.editable_profile(cx) else {
            self.load(cx);
            return Ok(());
        };
        let (Some(settings), Some(before)) = (self.settings, self.overrides.clone()) else {
            return Ok(());
        };
        let mut overrides = before.clone();
        {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return Ok(());
            };
            let keys: Vec<usize> = self.selection.iter().collect();
            edit(&settings, &mut overrides, &keys, keyboard);
            overrides.validate(&keyboard.capabilities)?;
        }
        if overrides == before {
            return Ok(());
        }
        self.overrides = Some(overrides.clone());
        self.revision = self.revision.wrapping_add(1);
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetKeyOverrides>(
                &Change {
                    profile,
                    value: before,
                },
                Change {
                    profile,
                    value: overrides,
                },
                Scope::Profile(profile),
                cx,
            )
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_err() {
                this.update(cx, Self::load).ok();
            }
        })
        .detach();
        cx.notify();
        Ok(())
    }

    fn editable_profile(&self, cx: &App) -> Option<Slot> {
        let profile = self.profile?;
        (self.store.read(cx).active() == Some(profile)).then_some(profile)
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let revision = self.revision;
        let settings = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeySettings, keys::Settings>(profile, cx)
        });
        let overrides = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeyOverrides, keys::Overrides>(profile, cx)
        });
        cx.spawn(async move |this, cx| {
            let settings = settings.await.ok();
            let overrides = overrides.await.ok();
            this.update(cx, |tuning, cx| {
                if tuning.revision != revision {
                    return;
                }
                tuning.profile = Some(profile);
                tuning.settings = settings;
                tuning.overrides = overrides;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Toggle {
    pub(crate) const fn is_on(self) -> bool {
        matches!(self, Self::On)
    }

    pub(crate) const fn is_mixed(self) -> bool {
        matches!(self, Self::Mixed)
    }
}

pub(crate) fn present(keyboard: &Keyboard) -> impl Iterator<Item = usize> + '_ {
    (0..keyboard.key_count()).filter(|key| keyboard.has_key(*key))
}

pub(crate) fn millimetres_text(millimetres: f32) -> SharedString {
    format!("{millimetres:.2}").into()
}

pub(crate) fn report(result: Result<(), Invalid>, window: &mut Window, cx: &mut App) {
    if let Err(invalid) = result {
        let message = format!("could not change the key settings: {invalid}");
        window.push_notification(Notification::error(message), cx);
    }
}

fn reading(values: impl IntoIterator<Item = f32>, span: Span) -> Reading {
    let values: Vec<f32> = values.into_iter().collect();
    let shown: Vec<SharedString> = values
        .iter()
        .map(|value| millimetres_text(*value))
        .collect();
    Reading {
        millimetres: values.first().copied().unwrap_or(span.min),
        mixed: shown.iter().any(|text| Some(text) != shown.first()),
        span,
    }
}

fn toggle(values: impl IntoIterator<Item = bool>) -> Toggle {
    match Selected::of(values) {
        Selected::Same(true) => Toggle::On,
        Selected::Same(false) | Selected::Nothing => Toggle::Off,
        Selected::Mixed(_) => Toggle::Mixed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_switches_read_on_off_or_mixed() {
        assert_eq!(toggle([true, true]), Toggle::On);
        assert_eq!(toggle([false]), Toggle::Off);
        assert_eq!(toggle([]), Toggle::Off);
        assert_eq!(toggle([true, false]), Toggle::Mixed);
    }

    #[test]
    fn distances_that_show_the_same_are_not_mixed() {
        let span = Span::actuation(4.0);
        let same = reading([1.601, 1.598], span);
        assert!(!same.mixed);
        assert!((same.millimetres - 1.601).abs() < f32::EPSILON);
        assert!(reading([1.6, 1.65], span).mixed);
        assert!((reading([], span).millimetres - span.min).abs() < f32::EPSILON);
    }
}
