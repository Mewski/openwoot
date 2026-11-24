mod effective;
mod sections;
mod visibility;

use std::{
    collections::BTreeMap,
    mem,
    time::{Duration, Instant},
};

use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, Modifiers, ParentElement as _, Render,
    Subscription, Task, Window,
    component::{
        WindowExt as _,
        notification::Notification,
        slider::{SliderEvent, SliderState},
    },
    prelude::FluentBuilder as _,
};
use openwoot_protocol::{
    configuration::{Rejection, Validate as _},
    endpoints::{
        GetKeyOverrides, GetKeySettings, GetSensorValues, SetKeyOverrides, SetKeySettings,
    },
    keys::{self, Override, RapidTrigger, RapidTriggerOverride, ScanMode, TapDuration},
    profile::{Change, Slot},
    sensors::{self, Reading},
};

use self::{effective::RapidTriggerMode, visibility::Visibility};
use crate::{
    device::Failure,
    keyboard::Selection,
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
};

const MIN_DISTANCE_MM: f32 = 0.1;
const DISTANCE_STEP_MM: f32 = 0.05;
const SENSITIVITY_RANGE_MM: f32 = 2.5;
const FALLBACK_TRAVEL_MM: f32 = 4.0;
const POLL_INTERVAL: Duration = Duration::from_millis(33);
const RETRY_INTERVAL: Duration = Duration::from_millis(500);
const PRESSED_TRAVEL: u16 = 40;

pub(crate) struct PerformancePage {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<keys::Settings>,
    overrides: Option<keys::Overrides>,
    revision: u64,
    selection: Selection,
    labels: KeyLabels,
    travel: sensors::Values,
    pressed: Option<usize>,
    travel_unsupported: bool,
    actuation: Entity<SliderState>,
    press: Entity<SliderState>,
    release: Entity<SliderState>,
    visibility: Visibility,
    polling: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyLabels {
    Actuation,
    RapidTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sensitivity {
    Press,
    Release,
}

struct Target {
    travel: f32,
    actuation: f32,
    press: f32,
    release: f32,
}

impl PerformancePage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let distance = |cx: &mut Context<Self>| {
            cx.new(|_| {
                SliderState::new()
                    .min(MIN_DISTANCE_MM)
                    .max(FALLBACK_TRAVEL_MM)
                    .step(DISTANCE_STEP_MM)
                    .default_value(MIN_DISTANCE_MM)
            })
        };
        let actuation = distance(cx);
        let press = distance(cx);
        let release = distance(cx);
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected => {
                    this.selection.clear();
                    this.load(window, cx);
                }
                StoreEvent::ProfileChanged | StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected | StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &actuation,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => this.set_actuation(value.end(), window, cx),
                },
            ),
            cx.subscribe_in(
                &press,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
                        this.set_sensitivity(Sensitivity::Press, value.end(), window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &release,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
                        this.set_sensitivity(Sensitivity::Release, value.end(), window, cx);
                    }
                },
            ),
        ];
        Self {
            store,
            profile: None,
            settings: None,
            overrides: None,
            revision: 0,
            selection: Selection::default(),
            labels: KeyLabels::Actuation,
            travel: sensors::Values::new(),
            pressed: None,
            travel_unsupported: false,
            actuation,
            press,
            release,
            visibility: Visibility::new(Instant::now()),
            polling: false,
            _subscriptions: subscriptions,
        }
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        cx.spawn_in(window, async move |this, cx| {
            let settings = settings.await.ok();
            let overrides = overrides.await.ok();
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    return;
                }
                page.profile = Some(profile);
                page.settings = settings;
                page.overrides = overrides;
                page.sync_sliders(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn keep_polling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.polling {
            self.visibility.rendered(Instant::now());
            return;
        }
        self.polling = true;
        self.visibility.restart(Instant::now());
        cx.defer_in(window, Self::load);
        let device = self.store.read(cx).device();
        cx.spawn(async move |this, cx| {
            loop {
                let started = Instant::now();
                let shown = this
                    .update(cx, |page, _| page.visibility.is_shown(started))
                    .unwrap_or(false);
                if !shown {
                    this.update(cx, |page, _| page.stop_polling()).ok();
                    return;
                }
                let reading = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await;
                let pause = if reading.is_ok() {
                    POLL_INTERVAL.saturating_sub(started.elapsed())
                } else {
                    RETRY_INTERVAL
                };
                if this
                    .update(cx, |page, cx| page.show_travel(reading, cx))
                    .is_err()
                {
                    return;
                }
                cx.background_executor().timer(pause).await;
            }
        })
        .detach();
    }

    fn stop_polling(&mut self) {
        self.polling = false;
        self.travel.clear();
        self.pressed = None;
    }

    fn show_travel(&mut self, reading: Result<sensors::Values, Failure>, cx: &mut Context<Self>) {
        let mut changed = false;
        let unsupported = matches!(reading, Err(Failure::Rejected(Rejection::Unsupported)));
        if unsupported != self.travel_unsupported {
            self.travel_unsupported = unsupported;
            changed = true;
        }
        if let Ok(values) = reading
            && values != self.travel
        {
            if let Some(key) = deepest(&values) {
                self.pressed = Some(key);
            }
            self.travel = values;
            changed = true;
        }
        let now = Instant::now();
        if changed || self.visibility.redraw_due(now) {
            self.visibility.notified(now);
            cx.notify();
        }
    }

    fn click_key(
        &mut self,
        key: usize,
        modifiers: Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection.click(key, modifiers);
        self.selection_changed(window, cx);
    }

    fn select_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            self.selection.select_all(keyboard);
        }
        self.selection_changed(window, cx);
    }

    fn clear_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selection_changed(window, cx);
    }

    fn selection_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn show_labels(&mut self, labels: KeyLabels, cx: &mut Context<Self>) {
        self.labels = labels;
        cx.notify();
    }

    fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| self.target(keyboard))
        else {
            return;
        };
        let sensitivity = target.travel.min(SENSITIVITY_RANGE_MM);
        set_range(
            &self.actuation,
            target.travel.max(target.actuation),
            target.actuation,
            window,
            cx,
        );
        set_range(
            &self.press,
            sensitivity.max(target.press),
            target.press,
            window,
            cx,
        );
        set_range(
            &self.release,
            sensitivity.max(target.release),
            target.release,
            window,
            cx,
        );
    }

    fn target(&self, keyboard: &Keyboard) -> Option<Target> {
        let settings = self.settings.as_ref()?;
        let overrides = self.overrides.as_ref()?;
        let (first, travel, entry) = match self.selection.first() {
            None => {
                let key = reference_key(keyboard);
                (key, keyboard.travel_mm(key), Override::default())
            }
            Some(first) => (
                first,
                self.selection
                    .iter()
                    .map(|key| keyboard.travel_mm(key))
                    .fold(f32::INFINITY, f32::min),
                overrides.get(first).copied().unwrap_or_default(),
            ),
        };
        let parameters = effective::parameters(settings, &entry);
        Some(Target {
            travel,
            actuation: keyboard.distance_mm(first, effective::actuation(settings, &entry)),
            press: keyboard.distance_mm(first, parameters.press),
            release: keyboard.distance_mm(first, parameters.release.unwrap_or(parameters.press)),
        })
    }

    fn editable_profile(&self, cx: &App) -> Option<Slot> {
        let profile = self.profile?;
        (self.store.read(cx).active() == Some(profile)).then_some(profile)
    }

    fn change_settings(
        &mut self,
        edit: impl FnOnce(&mut keys::Settings, &Keyboard),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.editable_profile(cx) else {
            self.load(window, cx);
            return;
        };
        let Some(mut settings) = self.settings else {
            return;
        };
        let checked = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            edit(&mut settings, keyboard);
            settings.validate(&keyboard.capabilities)
        };
        match checked {
            Err(invalid) => {
                let message = format!("could not change the key settings: {invalid}");
                window.push_notification(Notification::error(message), cx);
            }
            Ok(()) if self.settings == Some(settings) => {}
            Ok(()) => {
                self.settings = Some(settings);
                self.revision = self.revision.wrapping_add(1);
                let applied = self.store.update(cx, |store, cx| {
                    store.apply::<SetKeySettings>(
                        Change {
                            profile,
                            value: settings,
                        },
                        Scope::Profile(profile),
                        cx,
                    )
                });
                Self::reload_on_failure(applied, window, cx);
            }
        }
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn change_overrides(
        &mut self,
        edit: impl FnOnce(&mut keys::Overrides, &Selection, &keys::Settings, &Keyboard),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.editable_profile(cx) else {
            self.load(window, cx);
            return;
        };
        let (Some(settings), Some(mut overrides)) = (self.settings, self.overrides.clone()) else {
            return;
        };
        let checked = {
            let Some(keyboard) = self.store.read(cx).keyboard() else {
                return;
            };
            edit(&mut overrides, &self.selection, &settings, keyboard);
            overrides.validate(&keyboard.capabilities)
        };
        match checked {
            Err(invalid) => {
                let message = format!("could not change the keys' own settings: {invalid}");
                window.push_notification(Notification::error(message), cx);
            }
            Ok(()) if self.overrides.as_ref() == Some(&overrides) => {}
            Ok(()) => {
                self.overrides = Some(overrides.clone());
                self.revision = self.revision.wrapping_add(1);
                let applied = self.store.update(cx, |store, cx| {
                    store.apply::<SetKeyOverrides>(
                        Change {
                            profile,
                            value: overrides,
                        },
                        Scope::Profile(profile),
                        cx,
                    )
                });
                Self::reload_on_failure(applied, window, cx);
            }
        }
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn reload_on_failure(
        applied: Task<Result<(), Failure>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            if applied.await.is_err() {
                this.update_in(cx, Self::load).ok();
            }
        })
        .detach();
    }

    fn set_actuation(&mut self, millimetres: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            self.change_settings(
                |settings, keyboard| {
                    settings.actuation =
                        keyboard.distance_from_mm(reference_key(keyboard), millimetres);
                },
                window,
                cx,
            );
        } else {
            self.change_overrides(
                |overrides, selection, _, keyboard| {
                    for key in selection.iter() {
                        if let Some(entry) = overrides.get_mut(key) {
                            entry.actuation = Some(keyboard.distance_from_mm(key, millimetres));
                        }
                    }
                },
                window,
                cx,
            );
        }
    }

    fn reset_actuation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_overrides(
            |overrides, selection, _, _| {
                for key in selection.iter() {
                    if let Some(entry) = overrides.get_mut(key) {
                        entry.actuation = None;
                    }
                }
            },
            window,
            cx,
        );
    }

    fn reset_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.change_overrides(
            |overrides, selection, _, _| {
                for key in selection.iter() {
                    if let Some(entry) = overrides.get_mut(key) {
                        *entry = Override::default();
                    }
                }
            },
            window,
            cx,
        );
    }

    fn set_rapid_trigger_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_settings(
            |settings, _| settings.rapid_trigger_enabled = enabled,
            window,
            cx,
        );
    }

    fn set_rapid_trigger_mode(
        &mut self,
        mode: RapidTriggerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_overrides(
            |overrides, selection, settings, _| {
                for key in selection.iter() {
                    if let Some(entry) = overrides.get_mut(key) {
                        entry.rapid_trigger =
                            mode.apply(entry.rapid_trigger, settings.rapid_trigger);
                    }
                }
            },
            window,
            cx,
        );
    }

    fn change_rapid_trigger(
        &mut self,
        edit: impl Fn(&mut RapidTrigger, usize, &Keyboard),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selection.is_empty() {
            self.change_settings(
                |settings, keyboard| {
                    edit(
                        &mut settings.rapid_trigger,
                        reference_key(keyboard),
                        keyboard,
                    );
                },
                window,
                cx,
            );
        } else {
            self.change_overrides(
                |overrides, selection, _, keyboard| {
                    for key in selection.iter() {
                        if let Some(Override {
                            rapid_trigger: RapidTriggerOverride::With(own),
                            ..
                        }) = overrides.get_mut(key)
                        {
                            edit(own, key, keyboard);
                        }
                    }
                },
                window,
                cx,
            );
        }
    }

    fn set_sensitivity(
        &mut self,
        sensitivity: Sensitivity,
        millimetres: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_rapid_trigger(
            |rapid_trigger, key, keyboard| {
                let distance = keyboard.distance_from_mm(key, millimetres);
                match sensitivity {
                    Sensitivity::Press => rapid_trigger.press = distance,
                    Sensitivity::Release => rapid_trigger.release = Some(distance),
                }
            },
            window,
            cx,
        );
    }

    fn set_split(&mut self, split: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.change_rapid_trigger(
            |rapid_trigger, _, _| {
                rapid_trigger.release = split.then_some(rapid_trigger.press);
            },
            window,
            cx,
        );
    }

    fn set_continuous(&mut self, continuous: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.change_rapid_trigger(
            |rapid_trigger, _, _| rapid_trigger.continuous = continuous,
            window,
            cx,
        );
    }

    fn set_tachyon(&mut self, tachyon: bool, window: &mut Window, cx: &mut Context<Self>) {
        let scan_mode = if tachyon {
            ScanMode::Parallel
        } else {
            ScanMode::Single
        };
        self.change_settings(|settings, _| settings.scan_mode = scan_mode, window, cx);
    }

    fn set_tap_duration(
        &mut self,
        tap_duration: TapDuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_settings(
            |settings, _| settings.tap_duration = tap_duration,
            window,
            cx,
        );
    }

    fn set_keyboard_in_gamepad_mode(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_settings(
            |settings, _| settings.keyboard_in_gamepad_mode = enabled,
            window,
            cx,
        );
    }
}

impl Render for PerformancePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.keep_polling(window, cx);
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return page;
        };
        if self.settings.is_none() || self.overrides.is_none() {
            return page.when(self.profile.is_some(), |page| {
                page.child(panel::note("Key settings unavailable", cx))
            });
        }
        page.child(self.keyboard(keyboard, window, cx))
            .child(self.heading(cx))
            .child(self.cards(keyboard, cx))
    }
}

fn set_range(
    slider: &Entity<SliderState>,
    max: f32,
    value: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let max = max.max(MIN_DISTANCE_MM);
    slider.update(cx, |state, cx| {
        let current = mem::replace(state, SliderState::new());
        *state = current.min(MIN_DISTANCE_MM).max(max);
        state.set_value(value.clamp(MIN_DISTANCE_MM, max), window, cx);
    });
}

fn reference_key(keyboard: &Keyboard) -> usize {
    let mut models = BTreeMap::<Option<u8>, (usize, usize)>::new();
    for key in (0..keyboard.key_count()).filter(|key| keyboard.has_key(*key)) {
        let model = keyboard
            .switches
            .as_ref()
            .and_then(|switches| switches.of_key(key))
            .map(|switch| switch.model);
        models.entry(model).or_insert((0, key)).0 += 1;
    }
    models
        .into_values()
        .max_by_key(|(count, _)| *count)
        .map_or(0, |(_, key)| key)
}

fn deepest(values: &sensors::Values) -> Option<usize> {
    values
        .iter()
        .enumerate()
        .filter_map(|(key, travel)| {
            travel
                .filter(|travel| *travel >= PRESSED_TRAVEL)
                .map(|travel| (key, travel))
        })
        .max_by_key(|(_, travel)| *travel)
        .map(|(key, _)| key)
}
