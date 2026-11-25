mod chart;
mod controls;
mod curve;
mod glyph;
mod output;
mod palette;
mod system;
mod tester;
mod view;

use std::time::{Duration, Instant};

use gpui_kit::{
    AppContext as _, Context, Entity, SharedString, Subscription, Task, Window,
    component::{
        WindowExt as _,
        input::{InputEvent, InputState},
        notification::Notification,
        slider::{SliderEvent, SliderState},
    },
};
use openwoot_protocol::{
    configuration::MAX_KEYS,
    device::{self, Mode, PollingRate},
    endpoints::{
        GetDeviceSettings, GetGamepadMapping, GetGamepadSettings, GetGamepadState, GetKeyOverrides,
        GetKeySettings, GetSensorValues, Restart, SetDeviceSettings, SetGamepadDetection,
        SetGamepadMapping, SetGamepadSettings, SetKeySettings,
    },
    gamepad::{self, Control},
    keys::{self, Action, Distance},
    profile::{Change, Slot},
    sensors::{self, Reading},
};

use self::{
    chart::Input,
    controls::Controller,
    curve::DEFAULT_UP_BIAS,
    output::{Output, Priorities},
};
use crate::{
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const SENSOR_TRAVEL: u32 = 4095;
const STATE_EVERY_POLLS: u32 = 15;
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Setup,
    Configuration,
    Tester,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carried {
    Control(Control),
    Binding { key: usize, control: Control },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceChange {
    Mode(Mode),
    Polling(PollingRate),
}

pub(crate) struct GamepadPage {
    store: Entity<Store>,
    view: View,
    device: Option<device::SettingsState>,
    mapping: Option<gamepad::Mapping>,
    key_settings: Option<keys::Settings>,
    overrides: Option<keys::Overrides>,
    analog: Option<gamepad::Settings>,
    edit_origin: Option<gamepad::Settings>,
    hovered_point: Option<usize>,
    dragged_point: Option<usize>,
    carried_from: Option<usize>,
    up_bias: Entity<SliderState>,
    angle: Entity<InputState>,
    detecting: bool,
    detection_pending: bool,
    gamepad_state: Option<gamepad::State>,
    travel: Option<sensors::Values>,
    priorities: Priorities,
    rendered_at: Instant,
    poll: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl GamepadPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let up_bias = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(f32::from(DEFAULT_UP_BIAS))
        });
        let angle =
            cx.new(|cx| InputState::new(window, cx).default_value(curve::degrees(DEFAULT_UP_BIAS)));
        let subscriptions =
            vec![
                cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                    StoreEvent::Connected => {
                        this.detecting = false;
                        this.load(window, cx);
                    }
                    StoreEvent::ProfileChanged | StoreEvent::Reloaded => this.load(window, cx),
                    StoreEvent::Disconnected => {
                        this.detecting = false;
                        this.travel = None;
                        this.gamepad_state = None;
                        cx.notify();
                    }
                    StoreEvent::Failed(_) => {}
                }),
                cx.subscribe_in(
                    &up_bias,
                    window,
                    |this, _, event: &SliderEvent, window, cx| match event {
                        SliderEvent::Change(value) => {
                            let percent = curve::point_value(value.end()).min(100);
                            this.slide_bias(percent, window, cx);
                        }
                        SliderEvent::Release(_) => this.finish_edit(window, cx),
                    },
                ),
                cx.subscribe_in(&angle, window, |this, _, event: &InputEvent, window, cx| {
                    match event {
                        InputEvent::PressEnter { .. } | InputEvent::Blur => {
                            this.commit_angle(window, cx);
                        }
                        InputEvent::Change | InputEvent::Focus => {}
                    }
                }),
                cx.on_release(|page: &mut Self, cx| {
                    if page.detecting {
                        let stop = page
                            .store
                            .read(cx)
                            .device()
                            .call::<SetGamepadDetection>(false);
                        cx.background_spawn(stop).detach();
                    }
                }),
            ];
        let mut page = Self {
            store,
            view: View::Setup,
            device: None,
            mapping: None,
            key_settings: None,
            overrides: None,
            analog: None,
            edit_origin: None,
            hovered_point: None,
            dragged_point: None,
            carried_from: None,
            up_bias,
            angle,
            detecting: false,
            detection_pending: false,
            gamepad_state: None,
            travel: None,
            priorities: Priorities::default(),
            rendered_at: Instant::now(),
            poll: None,
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let (device, mapping, key_settings, overrides, analog) =
            self.store.update(cx, |store, cx| {
                (
                    store.fetch::<GetDeviceSettings, _>((), cx),
                    store.fetch::<GetGamepadMapping, _>(profile, cx),
                    store.fetch::<GetKeySettings, _>(profile, cx),
                    store.fetch::<GetKeyOverrides, _>(profile, cx),
                    store.fetch::<GetGamepadSettings, _>(profile, cx),
                )
            });
        cx.spawn_in(window, async move |this, cx| {
            let device = device.await.ok();
            let mapping = mapping.await.ok();
            let key_settings = key_settings.await.ok();
            let overrides = overrides.await.ok();
            let analog = analog.await.ok();
            this.update_in(cx, |page, window, cx| {
                page.device = device;
                page.mapping = mapping;
                page.key_settings = key_settings;
                page.overrides = overrides;
                page.analog = analog;
                page.edit_origin = None;
                page.dragged_point = None;
                page.sync_bias(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn profile(&self, cx: &Context<Self>) -> Option<Slot> {
        self.store.read(cx).active()
    }

    fn controller(&self) -> Option<Controller> {
        self.device
            .and_then(|state| Controller::of(state.chosen.mode))
    }

    fn running_controller(&self) -> Option<Controller> {
        self.device
            .and_then(|state| Controller::of(state.active.mode))
    }

    fn restart_pending(&self) -> bool {
        self.device
            .is_some_and(|state| state.active != state.chosen)
    }

    fn shown_controller(&self) -> Controller {
        self.controller().unwrap_or(Controller::PREFERRED)
    }

    fn sync_bias(&self, window: &mut Window, cx: &mut Context<Self>) {
        let bias = self.analog.map_or(DEFAULT_UP_BIAS, |settings| {
            settings.left_stick_up_bias.percent
        });
        self.up_bias.update(cx, |slider, cx| {
            slider.set_value(f32::from(bias), window, cx);
        });
        self.angle.update(cx, |field, cx| {
            field.set_value(curve::degrees(bias), window, cx);
        });
    }

    fn commit_angle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match curve::bias_of_degrees(&self.angle.read(cx).value()) {
            Some(percent) => self.change_analog(
                |settings| settings.left_stick_up_bias.percent = percent,
                window,
                cx,
            ),
            None => self.sync_bias(window, cx),
        }
    }

    fn choose_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        if self.view == View::Tester {
            self.gamepad_state = None;
            if self.detecting {
                self.set_detection(false, cx);
            }
        }
        self.view = view;
        self.hovered_point = None;
        if view == View::Setup {
            self.poll = None;
            self.travel = None;
        } else if self.poll.is_none() {
            self.start_polling(cx);
        }
        self.load(window, cx);
        cx.notify();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        let device = self.store.read(cx).device();
        self.poll = Some(cx.spawn(async move |this, cx| {
            let mut ticks: u32 = 0;
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let Ok((visible, testing)) = this.update(cx, |page, _| {
                    (
                        page.rendered_at.elapsed() < VISIBLE_WITHIN,
                        page.view == View::Tester,
                    )
                }) else {
                    return;
                };
                if !visible {
                    continue;
                }
                let travel = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await
                    .ok();
                let state = if testing && ticks.is_multiple_of(STATE_EVERY_POLLS) {
                    Some(
                        device
                            .configure::<GetGamepadState, gamepad::State>(())
                            .await
                            .ok(),
                    )
                } else {
                    None
                };
                ticks = ticks.wrapping_add(1);
                let updated = this.update(cx, |page, cx| {
                    page.take_travel(travel, cx);
                    if let Some(state) = state {
                        page.take_state(state, cx);
                    }
                });
                if updated.is_err() {
                    return;
                }
            }
        }));
    }

    fn take_travel(&mut self, travel: Option<sensors::Values>, cx: &mut Context<Self>) {
        if self.travel != travel {
            self.travel = travel;
            cx.notify();
        }
    }

    fn take_state(&mut self, state: Option<gamepad::State>, cx: &mut Context<Self>) {
        let detection = state
            .map(|state| state.detection)
            .filter(|_| !self.detection_pending)
            .unwrap_or(self.detecting);
        if self.gamepad_state != state || self.detecting != detection {
            self.gamepad_state = state;
            self.detecting = detection;
            cx.notify();
        }
    }

    fn confirm_device_change(
        &mut self,
        change: DeviceChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (title, action) = match change {
            DeviceChange::Mode(Mode::NoGamepad) => ("Disable Gamepad", "disable gamepad mode"),
            DeviceChange::Mode(_) if self.controller().is_none() => {
                ("Enable Gamepad", "enable gamepad mode")
            }
            DeviceChange::Mode(_) => (
                "Changing Gamepad Controller Type",
                "change the gamepad controller type",
            ),
            DeviceChange::Polling(_) => (
                "Changing Gamepad Polling Rate",
                "change the gamepad polling rate",
            ),
        };
        let unsaved = self
            .store
            .read(cx)
            .keyboard()
            .is_some_and(|keyboard| keyboard.unsaved);
        let outcome = if unsaved {
            "saves your changes and restarts"
        } else {
            "has to restart"
        };
        let description = SharedString::from(format!("When you {action}, your device {outcome}."));
        let page = cx.entity().downgrade();
        panel::confirm(
            panel::Confirmation::new(title, description, "Confirm"),
            move |window, cx| {
                page.update(cx, |page, cx| page.apply_device_change(change, window, cx))
                    .ok();
            },
            window,
            cx,
        );
    }

    fn apply_device_change(
        &mut self,
        change: DeviceChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(state) = current.await else {
                return;
            };
            let mut settings = state.chosen;
            match change {
                DeviceChange::Mode(mode) => settings.mode = mode,
                DeviceChange::Polling(rate) => settings.gamepad_polling = rate,
            }
            let Ok(applied) = this.update(cx, |page, cx| {
                page.store.update(cx, |store, cx| {
                    store.apply::<SetDeviceSettings>(settings, Scope::Keyboard, cx)
                })
            }) else {
                return;
            };
            if applied.await.is_err() {
                return;
            }
            this.update_in(cx, |page, window, cx| {
                page.device = Some(device::SettingsState {
                    chosen: settings,
                    ..state
                });
                cx.notify();
                page.restart(window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.store.update(cx, |store, cx| store.save(cx));
        let device = self.store.read(cx).device();
        cx.spawn_in(window, async move |_, cx| {
            if saved.await.is_err() {
                return;
            }
            if let Err(error) = device.call::<Restart>(()).await {
                let message = Notification::error(error.to_string());
                cx.update(|window, cx| window.push_notification(message, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn edit_bindings(&mut self, edit: impl FnOnce(&mut [Option<Control>]), cx: &mut Context<Self>) {
        let Some(profile) = self.profile(cx) else {
            return;
        };
        let key_count = self
            .store
            .read(cx)
            .keyboard()
            .map_or(0, Keyboard::key_count)
            .min(MAX_KEYS);
        let Some(mapping) = &mut self.mapping else {
            return;
        };
        let before = mapping.clone();
        while mapping.bindings.len() < key_count {
            if mapping.bindings.push(None).is_err() {
                break;
            }
        }
        edit(&mut mapping.bindings);
        let after = mapping.clone();
        let slots = after.bindings.len().max(before.bindings.len());
        let unchanged = (0..slots).all(|key| {
            after.bindings.get(key).copied().flatten()
                == before.bindings.get(key).copied().flatten()
        });
        if unchanged {
            cx.notify();
            return;
        }
        self.store
            .update(cx, |store, cx| {
                store.change::<SetGamepadMapping>(
                    &Change {
                        profile,
                        value: before,
                    },
                    Change {
                        profile,
                        value: after,
                    },
                    Scope::Profile(profile),
                    cx,
                )
            })
            .detach();
        cx.notify();
    }

    fn drop_on_key(&mut self, key: usize, carried: Carried, cx: &mut Context<Self>) {
        self.carried_from = None;
        self.edit_bindings(
            |bindings| match carried {
                Carried::Control(control) => {
                    if let Some(slot) = bindings.get_mut(key) {
                        *slot = Some(control);
                    }
                }
                Carried::Binding { key: from, control } => {
                    if from == key {
                        return;
                    }
                    let displaced = bindings.get(key).copied().flatten();
                    if let Some(slot) = bindings.get_mut(key) {
                        *slot = Some(control);
                    }
                    if let Some(slot) = bindings.get_mut(from) {
                        *slot = displaced;
                    }
                }
            },
            cx,
        );
    }

    fn drop_off_keyboard(&mut self, carried: Carried, cx: &mut Context<Self>) {
        self.carried_from = None;
        match carried {
            Carried::Binding { key, .. } => self.remove_binding(key, cx),
            Carried::Control(_) => cx.notify(),
        }
    }

    fn remove_binding(&mut self, key: usize, cx: &mut Context<Self>) {
        if self.binding(key).is_none() {
            return;
        }
        self.edit_bindings(
            |bindings| {
                if let Some(slot) = bindings.get_mut(key) {
                    *slot = None;
                }
            },
            cx,
        );
    }

    fn track_carried(&mut self, carried: Carried, cx: &mut Context<Self>) {
        let from = match carried {
            Carried::Binding { key, .. } => Some(key),
            Carried::Control(_) => None,
        };
        if self.carried_from != from {
            self.carried_from = from;
            cx.notify();
        }
    }

    fn set_exclusive(&mut self, exclusive: bool, cx: &mut Context<Self>) {
        let Some(profile) = self.profile(cx) else {
            return;
        };
        let Some(mapping) = &mut self.mapping else {
            return;
        };
        let before = mapping.clone();
        mapping.exclusive = exclusive;
        let after = mapping.clone();
        self.store
            .update(cx, |store, cx| {
                store.change::<SetGamepadMapping>(
                    &Change {
                        profile,
                        value: before,
                    },
                    Change {
                        profile,
                        value: after,
                    },
                    Scope::Profile(profile),
                    cx,
                )
            })
            .detach();
        cx.notify();
    }

    fn change_key_settings(
        &mut self,
        change: impl FnOnce(&mut keys::Settings) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.profile(cx) else {
            return;
        };
        let current = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeySettings, _>(profile, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(before) = current.await else {
                return;
            };
            let mut after = before;
            change(&mut after);
            this.update(cx, |page, cx| {
                page.key_settings = Some(after);
                page.store
                    .update(cx, |store, cx| {
                        store.change::<SetKeySettings>(
                            &Change {
                                profile,
                                value: before,
                            },
                            Change {
                                profile,
                                value: after,
                            },
                            Scope::Profile(profile),
                            cx,
                        )
                    })
                    .detach();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_keyboard_keys(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        if enabled {
            self.change_key_settings(
                move |settings| settings.keyboard_in_gamepad_mode = true,
                window,
                cx,
            );
            return;
        }
        let page = cx.entity().downgrade();
        panel::confirm(
            panel::Confirmation::new(
                "Disable keyboard keys",
                "You might lock yourself out of typing on this profile.",
                "Confirm",
            )
            .danger(),
            move |window, cx| {
                page.update(cx, |page, cx| {
                    page.change_key_settings(
                        |settings| settings.keyboard_in_gamepad_mode = false,
                        window,
                        cx,
                    );
                })
                .ok();
            },
            window,
            cx,
        );
    }

    fn change_analog(
        &mut self,
        change: impl FnOnce(&mut gamepad::Settings),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(before) = self.analog else {
            return;
        };
        let mut after = before;
        change(&mut after);
        self.analog = Some(after);
        self.sync_bias(window, cx);
        self.send_analog(before, window, cx);
    }

    fn send_analog(
        &mut self,
        before: gamepad::Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(profile), Some(value)) = (self.profile(cx), self.analog) else {
            return;
        };
        if value == before {
            cx.notify();
            return;
        }
        if let Err(problem) = curve::check(&value) {
            self.analog = Some(before);
            window.push_notification(Notification::error(problem.to_string()), cx);
            return;
        }
        self.store
            .update(cx, |store, cx| {
                store.change::<SetGamepadSettings>(
                    &Change {
                        profile,
                        value: before,
                    },
                    Change { profile, value },
                    Scope::Profile(profile),
                    cx,
                )
            })
            .detach();
        cx.notify();
    }

    fn slide_bias(&mut self, percent: u8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(settings) = &mut self.analog else {
            return;
        };
        if self.edit_origin.is_none() {
            self.edit_origin = Some(*settings);
        }
        settings.left_stick_up_bias.percent = percent;
        self.angle.update(cx, |field, cx| {
            field.set_value(curve::degrees(percent), window, cx);
        });
        cx.notify();
    }

    fn finish_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(before) = self.edit_origin.take() {
            self.send_analog(before, window, cx);
        }
    }

    fn on_chart(&mut self, input: Input, window: &mut Window, cx: &mut Context<Self>) {
        match input {
            Input::Hover(point) => {
                if self.dragged_point.is_none() && self.hovered_point != point {
                    self.hovered_point = point;
                    cx.notify();
                }
            }
            Input::Grab(point) => {
                self.dragged_point = Some(point);
                self.hovered_point = Some(point);
                self.edit_origin = self.analog;
                cx.notify();
            }
            Input::Move { travel, output } => {
                if let (Some(point), Some(settings)) = (self.dragged_point, &mut self.analog) {
                    curve::move_point(&mut settings.curve, point, travel, output);
                    cx.notify();
                }
            }
            Input::Release => {
                if self.dragged_point.take().is_some() {
                    self.finish_edit(window, cx);
                }
            }
        }
    }

    fn set_detection(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.detecting = enabled;
        self.detection_pending = true;
        let applied = self.store.update(cx, |store, cx| {
            store.apply::<SetGamepadDetection>(enabled, Scope::Runtime, cx)
        });
        cx.spawn(async move |this, cx| {
            let rejected = applied.await.is_err();
            this.update(cx, |page, cx| {
                page.detection_pending = false;
                if rejected && page.detecting == enabled {
                    page.detecting = !enabled;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn binding(&self, key: usize) -> Option<Control> {
        self.mapping
            .as_ref()
            .and_then(|mapping| mapping.bindings.get(key).copied().flatten())
    }

    fn key_travel(&self, key: usize) -> Option<u16> {
        self.travel
            .as_ref()
            .and_then(|travel| travel.get(key).copied().flatten())
    }

    fn threshold(&self, key: usize) -> u16 {
        let actuation = self
            .overrides
            .as_ref()
            .and_then(|overrides| overrides.get(key))
            .and_then(|entry| entry.actuation)
            .or_else(|| self.key_settings.map(|settings| settings.actuation));
        actuation.map_or(u16::MAX, travel_of)
    }

    fn controller_output(&mut self, keyboard: &Keyboard) -> Option<Output> {
        let controller = self.running_controller()?;
        let mapping = self.mapping.as_ref()?;
        self.travel.as_ref()?;
        let mapped = mapping
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(key, binding)| binding.map(|control| (key, control)));
        let remapped =
            keyboard
                .legends
                .iter()
                .enumerate()
                .filter_map(|(key, action)| match action {
                    Action::Gamepad(control) => Some((key, *control)),
                    _ => None,
                });
        let keys: Vec<(Control, u16, bool)> = mapped
            .chain(remapped)
            .map(|(key, control)| {
                let travel = self.key_travel(key).unwrap_or(0);
                (control, travel, travel >= self.threshold(key))
            })
            .collect();
        Some(output::compute(
            controller,
            keys,
            self.analog.as_ref(),
            &mut self.priorities,
        ))
    }
}

fn travel_of(distance: Distance) -> u16 {
    let travel = SENSOR_TRAVEL * u32::from(distance.fraction()) / u32::from(u16::MAX);
    u16::try_from(travel).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actuation_distances_become_sensor_travel() {
        assert_eq!(travel_of(Distance::FULL_TRAVEL), 4095);
        assert_eq!(travel_of(Distance::from_fraction(u16::MAX / 2)), 2047);
        assert_eq!(travel_of(Distance::from_fraction(0)), 0);
    }
}
