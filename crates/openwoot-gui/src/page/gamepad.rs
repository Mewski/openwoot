mod chart;
mod controls;
mod curve;
mod view;

use std::time::{Duration, Instant};

use gpui_kit::{
    AppContext as _, Context, Entity, Subscription, Task, Window,
    component::{
        WindowExt as _,
        button::ButtonVariant,
        notification::Notification,
        slider::{SliderEvent, SliderState},
    },
};
use openwoot_protocol::{
    configuration::MAX_KEYS,
    device,
    endpoints::{
        GetDeviceSettings, GetGamepadMapping, GetGamepadSettings, GetKeySettings, GetSensorValues,
        Restart, SetDeviceSettings, SetGamepadDetection, SetGamepadMapping, SetGamepadSettings,
        SetKeySettings,
    },
    gamepad::{self, CURVE_POINTS, Control},
    keys,
    profile::{Change, Slot},
    sensors::{self, Reading},
};

use self::{
    chart::Input,
    curve::{DEFAULT_UP_BIAS, POINT_SCALE},
};
use crate::{
    keyboard::Selection,
    store::{Keyboard, Scope, Store, StoreEvent},
};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Mapping,
    Analog,
    Test,
}

pub(crate) struct GamepadPage {
    store: Entity<Store>,
    view: View,
    device: Option<device::Settings>,
    started_with: Option<device::Settings>,
    mapping: Option<gamepad::Mapping>,
    key_settings: Option<keys::Settings>,
    analog: Option<gamepad::Settings>,
    selection: Selection,
    point: usize,
    dragging: Option<usize>,
    point_travel: Entity<SliderState>,
    point_output: Entity<SliderState>,
    up_bias: Entity<SliderState>,
    detecting: bool,
    travel: Option<sensors::Values>,
    rendered_at: Instant,
    poll: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl GamepadPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scale = |cx: &mut Context<Self>, max: f32, value: f32| {
            cx.new(|_| {
                SliderState::new()
                    .min(0.0)
                    .max(max)
                    .step(1.0)
                    .default_value(value)
            })
        };
        let point_travel = scale(cx, POINT_SCALE, 0.0);
        let point_output = scale(cx, POINT_SCALE, 0.0);
        let up_bias = scale(cx, 100.0, f32::from(DEFAULT_UP_BIAS));
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected => {
                    this.started_with = None;
                    this.detecting = false;
                    this.load(window, cx);
                }
                StoreEvent::ProfileChanged | StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected => {
                    this.detecting = false;
                    this.travel = None;
                    cx.notify();
                }
                StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &point_travel,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(value) => {
                        this.slide_point(Some(value.end()), None, window, cx);
                    }
                    SliderEvent::Release(_) => this.send_analog(window, cx),
                },
            ),
            cx.subscribe_in(
                &point_output,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(value) => {
                        this.slide_point(None, Some(value.end()), window, cx);
                    }
                    SliderEvent::Release(_) => this.send_analog(window, cx),
                },
            ),
            cx.subscribe_in(
                &up_bias,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    if let SliderEvent::Release(value) = event {
                        let percent = curve::point_value(value.end()).min(100);
                        this.change_analog(
                            |settings| settings.left_stick_up_bias = Some(percent),
                            window,
                            cx,
                        );
                    }
                },
            ),
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
            view: View::Mapping,
            device: None,
            started_with: None,
            mapping: None,
            key_settings: None,
            analog: None,
            selection: Selection::default(),
            point: 0,
            dragging: None,
            point_travel,
            point_output,
            up_bias,
            detecting: false,
            travel: None,
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
        let (device, mapping, key_settings, analog) = self.store.update(cx, |store, cx| {
            (
                store.fetch::<GetDeviceSettings, _>((), cx),
                store.fetch::<GetGamepadMapping, _>(profile, cx),
                store.fetch::<GetKeySettings, _>(profile, cx),
                store.fetch::<GetGamepadSettings, _>(profile, cx),
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let device = device.await.ok();
            let mapping = mapping.await.ok();
            let key_settings = key_settings.await.ok();
            let analog = analog.await.ok();
            this.update_in(cx, |page, window, cx| {
                page.device = device;
                if page.started_with.is_none() {
                    page.started_with = device;
                }
                page.mapping = mapping;
                page.key_settings = key_settings;
                page.analog = analog;
                page.dragging = None;
                page.sync_sliders(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn profile(&self, cx: &Context<Self>) -> Option<Slot> {
        self.store.read(cx).active()
    }

    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(settings) = self.analog else {
            return;
        };
        if let Some(point) = settings.curve.get(self.point) {
            let (travel, output) = (f32::from(point.travel), f32::from(point.output));
            self.point_travel
                .update(cx, |slider, cx| slider.set_value(travel, window, cx));
            self.point_output
                .update(cx, |slider, cx| slider.set_value(output, window, cx));
        }
        let bias = settings.left_stick_up_bias.unwrap_or(DEFAULT_UP_BIAS);
        self.up_bias.update(cx, |slider, cx| {
            slider.set_value(f32::from(bias), window, cx);
        });
    }

    fn choose_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        if self.view == View::Test {
            self.poll = None;
            self.travel = None;
            if self.detecting {
                self.set_detection(false, cx);
            }
        }
        self.view = view;
        if view == View::Test {
            self.start_polling(cx);
        }
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        let device = self.store.read(cx).device();
        self.poll = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let Ok(visible) =
                    this.update(cx, |page, _| page.rendered_at.elapsed() < VISIBLE_WITHIN)
                else {
                    return;
                };
                if !visible {
                    continue;
                }
                let reading = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await
                    .ok();
                if this
                    .update(cx, |page, cx| {
                        page.travel = reading;
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        }));
    }

    fn change_device(
        &mut self,
        change: impl FnOnce(&mut device::Settings) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(mut settings) = current.await else {
                return;
            };
            change(&mut settings);
            let Ok(applied) = this.update(cx, |page, cx| {
                page.store.update(cx, |store, cx| {
                    store.apply::<SetDeviceSettings>(settings, Scope::Keyboard, cx)
                })
            }) else {
                return;
            };
            if applied.await.is_ok() {
                this.update(cx, |page, cx| {
                    page.device = Some(settings);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn save_and_restart(&self, window: &mut Window, cx: &mut Context<Self>) {
        let save = self.store.update(cx, |store, cx| store.save(cx));
        let device = self.store.read(cx).device();
        cx.spawn_in(window, async move |_, cx| {
            if save.await.is_err() {
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

    fn change_mapping(
        &mut self,
        change: impl FnOnce(&mut gamepad::Mapping, usize),
        cx: &mut Context<Self>,
    ) {
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
        while mapping.bindings.len() < key_count {
            if mapping.bindings.push(None).is_err() {
                break;
            }
        }
        change(mapping, key_count);
        let value = mapping.clone();
        self.store
            .update(cx, |store, cx| {
                store.apply::<SetGamepadMapping>(
                    Change { profile, value },
                    Scope::Profile(profile),
                    cx,
                )
            })
            .detach();
        cx.notify();
    }

    fn assign(&mut self, control: Option<Control>, cx: &mut Context<Self>) {
        let keys: Vec<usize> = self.selection.iter().collect();
        self.change_mapping(
            |mapping, _| {
                for key in keys {
                    if let Some(binding) = mapping.bindings.get_mut(key) {
                        *binding = control;
                    }
                }
            },
            cx,
        );
    }

    fn select_mapped(&mut self, cx: &mut Context<Self>) {
        let mapped: Vec<usize> = self
            .mapping
            .iter()
            .flat_map(|mapping| mapping.bindings.iter().enumerate())
            .filter_map(|(key, binding)| binding.map(|_| key))
            .collect();
        self.selection.set(mapped);
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
            let Ok(mut value) = current.await else {
                return;
            };
            change(&mut value);
            let Ok(applied) = this.update(cx, |page, cx| {
                page.store.update(cx, |store, cx| {
                    store.apply::<SetKeySettings>(
                        Change { profile, value },
                        Scope::Profile(profile),
                        cx,
                    )
                })
            }) else {
                return;
            };
            if applied.await.is_ok() {
                this.update(cx, |page, cx| {
                    page.key_settings = Some(value);
                    cx.notify();
                })
                .ok();
            }
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
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title("Disable keyboard keys?")
                .description(
                    "This profile stops typing while the gamepad is on. Keep a profile or Fn \
                     key on it to switch back.",
                )
                .confirm()
                .ok_text("Disable")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| {
                        page.change_key_settings(
                            |settings| settings.keyboard_in_gamepad_mode = false,
                            window,
                            cx,
                        );
                    })
                    .ok();
                    true
                })
        });
    }

    fn change_analog(
        &mut self,
        change: impl FnOnce(&mut gamepad::Settings),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(settings) = &mut self.analog else {
            return;
        };
        change(settings);
        self.sync_sliders(window, cx);
        self.send_analog(window, cx);
    }

    fn send_analog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(profile), Some(value)) = (self.profile(cx), self.analog) else {
            return;
        };
        if let Err(problem) = curve::check(&value) {
            window.push_notification(Notification::error(problem.to_string()), cx);
            return;
        }
        self.store
            .update(cx, |store, cx| {
                store.apply::<SetGamepadSettings>(
                    Change { profile, value },
                    Scope::Profile(profile),
                    cx,
                )
            })
            .detach();
        cx.notify();
    }

    fn choose_point(&mut self, point: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.point = point.min(CURVE_POINTS - 1);
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn slide_point(
        &mut self,
        travel: Option<f32>,
        output: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self
            .analog
            .and_then(|settings| settings.curve.get(self.point).copied())
        else {
            return;
        };
        let travel = travel.map_or(current.travel, curve::point_value);
        let output = output.map_or(current.output, curve::point_value);
        self.move_point(travel, output, cx);
        let moved = self
            .analog
            .and_then(|settings| settings.curve.get(self.point).copied());
        if moved.is_some_and(|point| point.travel != travel) {
            self.sync_sliders(window, cx);
        }
    }

    fn move_point(&mut self, travel: u8, output: u8, cx: &mut Context<Self>) {
        if let Some(settings) = &mut self.analog {
            curve::move_point(&mut settings.curve, self.point, travel, output);
            cx.notify();
        }
    }

    fn on_chart(&mut self, input: Input, window: &mut Window, cx: &mut Context<Self>) {
        match input {
            Input::Grab(point) => {
                self.dragging = Some(point);
                self.choose_point(point, window, cx);
            }
            Input::Move { travel, output } => {
                if self.dragging.is_some() {
                    self.move_point(travel, output, cx);
                    self.sync_sliders(window, cx);
                }
            }
            Input::Release => {
                if self.dragging.take().is_some() {
                    self.send_analog(window, cx);
                }
            }
        }
    }

    fn set_detection(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.detecting = enabled;
        self.store
            .update(cx, |store, cx| {
                store.apply::<SetGamepadDetection>(enabled, Scope::Runtime, cx)
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

    fn common_binding(&self) -> Option<Control> {
        let mut bindings = self.selection.iter().map(|key| self.binding(key));
        let first = bindings.next()??;
        bindings
            .all(|binding| binding == Some(first))
            .then_some(first)
    }
}
