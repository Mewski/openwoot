mod chart;
mod controls;
mod curve;

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui_kit::{
    AppContext as _, Context, Div, Entity, FontWeight, Hsla, IntoElement, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Task, Window,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        notification::Notification,
        radio::{Radio, RadioGroup},
        slider::{Slider, SliderEvent, SliderState},
        switch::Switch,
        tab::{Tab, TabBar},
        v_flex,
    },
    div, px,
};
use openwoot_protocol::{
    configuration::MAX_KEYS,
    device::{self, Mode, PollingRate, Speed},
    endpoints::{
        GetDeviceSettings, GetGamepadMapping, GetGamepadSettings, GetKeySettings, GetSensorValues,
        Restart, SetDeviceSettings, SetGamepadDetection, SetGamepadMapping, SetGamepadSettings,
        SetKeySettings,
    },
    gamepad::{self, CURVE_POINTS, Control, Normalization},
    keys,
    profile::{Change, Slot},
    sensors::{self, Reading},
};

use self::{
    chart::{Chart, Input},
    controls::Support,
    curve::{DEFAULT_UP_BIAS, POINT_SCALE},
};
use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const POLL_INTERVAL: Duration = Duration::from_millis(33);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const KEYBOARD_MARGIN: f32 = 340.0;
const MODES: [(Mode, &str); 3] = [
    (Mode::NoGamepad, "Off"),
    (Mode::Xinput, "Xbox controller"),
    (Mode::Gamepad, "Classic controller"),
];
const POLLING_RATES: [(PollingRate, &str); 3] = [
    (PollingRate::Every4Milliseconds, "250 Hz"),
    (PollingRate::Every1Millisecond, "1000 Hz"),
    (PollingRate::Every125Microseconds, "8000 Hz"),
];
const POINT_NAMES: [&str; CURVE_POINTS] = ["Dead zone", "Knee 1", "Knee 2", "Saturation"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Setup,
    Configuration,
    Tester,
}

impl View {
    const ALL: [Self; 3] = [Self::Setup, Self::Configuration, Self::Tester];

    const fn title(self) -> &'static str {
        match self {
            Self::Setup => "Setup & Remap",
            Self::Configuration => "Configuration",
            Self::Tester => "Tester",
        }
    }
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
            view: View::Setup,
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
        if self.view == View::Tester {
            self.poll = None;
            self.travel = None;
            if self.detecting {
                self.set_detection(false, cx);
            }
        }
        self.view = view;
        if view == View::Tester {
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
            let message = match device.call::<Restart>(()).await {
                Ok(()) => {
                    Notification::info("The keyboard restarts with the new controller settings")
                }
                Err(error) => Notification::error(error.to_string()),
            };
            cx.update(|window, cx| window.push_notification(message, cx))
                .ok();
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
                .title("Disable keyboard keys")
                .description(
                    "While the gamepad is on, this profile no longer types. Before proceeding, \
                     make sure a Mode key, a profile key or a Fn layer shortcut on this profile \
                     switches to a profile that has keyboard keys enabled.",
                )
                .confirm()
                .ok_text("Disable keyboard keys")
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

    fn setup_view(&self, keyboard: &Keyboard, window: &Window, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap_4()
            .child(self.controller_section(cx))
            .child(self.mapping_section(keyboard, window, cx))
            .child(self.keys_section(cx))
    }

    fn controller_section(&self, cx: &mut Context<Self>) -> Div {
        let device = self.device;
        let mode = device.map(|settings| settings.mode);
        let polling = device.map(|settings| settings.gamepad_polling);
        let full_speed = device.is_some_and(|settings| settings.speed == Speed::Full);
        let gamepad_on = mode.is_some_and(|mode| mode != Mode::NoGamepad);
        let pending = matches!(
            (device, self.started_with),
            (Some(now), Some(started)) if (now.mode, now.gamepad_polling) != (started.mode, started.gamepad_polling)
        );
        let mut section = panel::section(
            "Controller",
            Some(
                "These settings affect every profile that uses the gamepad. The keyboard has to \
                 restart before they take effect."
                    .into(),
            ),
            cx,
        )
        .child(panel::row(
            "Controller type",
            Some("What the keyboard shows itself as to the computer, next to the keyboard.".into()),
            RadioGroup::horizontal("gamepad-mode")
                .children(MODES.map(|(_, name)| Radio::new(name).label(name)))
                .selected_index(
                    mode.and_then(|mode| MODES.iter().position(|entry| entry.0 == mode)),
                )
                .disabled(device.is_none())
                .on_change(cx.listener(|this, index: &usize, window, cx| {
                    if let Some((mode, _)) = MODES.get(*index) {
                        let mode = *mode;
                        this.change_device(move |settings| settings.mode = mode, window, cx);
                    }
                })),
            cx,
        ))
        .child(panel::row(
            "Polling rate",
            Some(
                if full_speed {
                    "How often the keyboard sends gamepad inputs. 8000 Hz needs high-speed USB; \
                     turn off Force full speed in Keyboard settings to use it."
                } else {
                    "How often the keyboard sends gamepad inputs. Only use 250 Hz if you \
                     experience compatibility or stability issues."
                }
                .into(),
            ),
            RadioGroup::horizontal("gamepad-polling")
                .children(POLLING_RATES.map(|(rate, name)| {
                    Radio::new(name)
                        .label(name)
                        .disabled(full_speed && rate == PollingRate::Every125Microseconds)
                }))
                .selected_index(
                    polling.and_then(|polling| {
                        POLLING_RATES.iter().position(|entry| entry.0 == polling)
                    }),
                )
                .disabled(!gamepad_on)
                .on_change(cx.listener(|this, index: &usize, window, cx| {
                    if let Some((rate, _)) = POLLING_RATES.get(*index) {
                        let rate = *rate;
                        this.change_device(
                            move |settings| settings.gamepad_polling = rate,
                            window,
                            cx,
                        );
                    }
                })),
            cx,
        ));
        if cfg!(target_os = "macos") && mode == Some(Mode::Xinput) {
            section = section.child(panel::note(
                "macOS does not support Xbox controller mode; use Classic controller mode instead.",
                cx,
            ));
        }
        if pending {
            section = section.child(panel::row(
                "Restart needed",
                Some(
                    "The keyboard still runs with the previous controller settings. Saving \
                     stores every unsaved change before the keyboard restarts."
                        .into(),
                ),
                Button::new("gamepad-restart")
                    .primary()
                    .label("Save and restart")
                    .on_click(cx.listener(|this, _, window, cx| this.save_and_restart(window, cx))),
                cx,
            ));
        }
        section
    }

    fn mapping_section(&self, keyboard: &Keyboard, window: &Window, cx: &mut Context<Self>) -> Div {
        let mode = self
            .device
            .map_or(Mode::NoGamepad, |settings| settings.mode);
        let theme = cx.theme();
        let mapped_fill = theme.primary.opacity(0.3);
        let unsupported_fill = theme.danger.opacity(0.3);
        let muted = theme.muted_foreground;
        let page = cx.entity().downgrade();
        let board = KeyboardView::new("gamepad-mapping", keyboard, |key| {
            mapping_face(
                keyboard,
                key,
                self.binding(key),
                mode,
                mapped_fill,
                unsupported_fill,
            )
        })
        .selection(&self.selection)
        .fit_width(window.viewport_size().width - px(KEYBOARD_MARGIN))
        .on_key_click(move |key, event, _, cx| {
            page.update(cx, |page, cx| {
                page.selection.click(key, event.modifiers());
                cx.notify();
            })
            .ok();
        });
        let selected = self.selection.len();
        let none_selected = self.selection.is_empty() || self.mapping.is_none();
        let toolbar =
            h_flex()
                .gap_2()
                .flex_wrap()
                .items_center()
                .child(div().text_sm().text_color(muted).child(SharedString::from(
                    match selected {
                        0 => "No key selected".to_owned(),
                        1 => "1 key selected".to_owned(),
                        count => format!("{count} keys selected"),
                    },
                )))
                .child(div().flex_1())
                .child(
                    Button::new("select-mapped")
                        .outline()
                        .small()
                        .label("Select mapped keys")
                        .on_click(cx.listener(|this, _, _, cx| this.select_mapped(cx))),
                )
                .child(
                    Button::new("select-all")
                        .outline()
                        .small()
                        .label("Select all")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(keyboard) = this.store.read(cx).keyboard().cloned() {
                                this.selection.select_all(&keyboard);
                            }
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("clear-selection")
                        .outline()
                        .small()
                        .label("Clear selection")
                        .disabled(self.selection.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.selection.clear();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("remove-control")
                        .danger()
                        .small()
                        .label("Remove control")
                        .disabled(none_selected)
                        .on_click(cx.listener(|this, _, _, cx| this.assign(None, cx))),
                );
        panel::section(
            "Controller mapping",
            Some(
                "Select keys on the keyboard, then pick the control they drive. Shift-, Ctrl- or \
                 Cmd-click adds keys to the selection. Keys on red do not reach the computer with \
                 the current controller type."
                    .into(),
            ),
            cx,
        )
        .child(div().w_full().overflow_hidden().child(board))
        .child(toolbar)
        .child(self.control_picker(none_selected, cx))
    }

    fn control_picker(&self, none_selected: bool, cx: &mut Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let common = self.common_binding();
        let mut groups = v_flex().gap_3();
        for group in &controls::GROUPS {
            let classic_only = group
                .controls
                .iter()
                .all(|control| controls::support(*control) == Support::ClassicOnly);
            let title = if classic_only {
                SharedString::from(format!("{} · Classic controller only", group.title))
            } else {
                SharedString::from(group.title)
            };
            let buttons = group.controls.iter().map(|control| {
                let control = *control;
                let (short, long) = usage::control(control);
                let tooltip = match controls::support(control) {
                    Support::Both => SharedString::from(long),
                    Support::XboxOnly => format!("{long} · Xbox controller only").into(),
                    Support::ClassicOnly => format!("{long} · Classic controller only").into(),
                };
                Button::new(SharedString::from(format!("control-{control:?}")))
                    .outline()
                    .small()
                    .label(short)
                    .tooltip(tooltip)
                    .selected(common == Some(control))
                    .disabled(none_selected)
                    .on_click(cx.listener(move |this, _, _, cx| this.assign(Some(control), cx)))
            });
            groups = groups.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(muted)
                            .child(title),
                    )
                    .child(h_flex().gap_1().flex_wrap().children(buttons)),
            );
        }
        groups
    }

    fn common_binding(&self) -> Option<Control> {
        let mut bindings = self.selection.iter().map(|key| self.binding(key));
        let first = bindings.next()??;
        bindings
            .all(|binding| binding == Some(first))
            .then_some(first)
    }

    fn keys_section(&self, cx: &mut Context<Self>) -> Div {
        let exclusive = self.mapping.as_ref().map(|mapping| mapping.exclusive);
        let keyboard_keys = self
            .key_settings
            .map(|settings| settings.keyboard_in_gamepad_mode);
        panel::section("Keys", None, cx)
            .child(panel::row(
                "Gamepad mapping override",
                Some("Disable the keyboard input on keys with gamepad bindings on them.".into()),
                Switch::new("gamepad-exclusive")
                    .checked(exclusive.unwrap_or(false))
                    .disabled(exclusive.is_none())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        let exclusive = *checked;
                        this.change_mapping(|mapping, _| mapping.exclusive = exclusive, cx);
                    })),
                cx,
            ))
            .child(panel::row(
                "Enable keyboard keys",
                Some(
                    "Keep the keys typing while the gamepad is on. Disable them to prevent them \
                     from interfering with your controller output keys."
                        .into(),
                ),
                Switch::new("keyboard-in-gamepad-mode")
                    .checked(keyboard_keys.unwrap_or(true))
                    .disabled(keyboard_keys.is_none())
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        this.set_keyboard_keys(*checked, window, cx);
                    })),
                cx,
            ))
    }

    fn configuration_view(&self, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap_4()
            .child(self.curve_section(cx))
            .child(self.joystick_section(cx))
    }

    fn curve_section(&self, cx: &mut Context<Self>) -> Div {
        let section = panel::section(
            "Analog curve",
            Some(
                "How a key press translates to the analog signal of the controller. Choose a \
                 preset or drag the points to suit your preferences."
                    .into(),
            ),
            cx,
        );
        let Some(settings) = self.analog else {
            return section.child(panel::note("Reading the analog settings", cx));
        };
        let current = curve::preset_of(settings.curve);
        let presets = h_flex()
            .gap_1()
            .flex_wrap()
            .children(curve::PRESETS.iter().enumerate().map(|(index, preset)| {
                let points = preset.curve;
                Button::new(SharedString::from(format!("curve-preset-{index}")))
                    .outline()
                    .small()
                    .label(preset.name)
                    .selected(current == Some(index))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.change_analog(|settings| settings.curve = points, window, cx);
                    }))
            }));
        let page = cx.entity().downgrade();
        let handler: chart::Handler = Rc::new(move |input, window, cx| {
            page.update(cx, |page, cx| page.on_chart(input, window, cx))
                .ok();
        });
        let plot = chart::chart(
            Chart {
                curve: settings.curve,
                selected: Some(self.point),
                markers: Vec::new(),
                handler: Some(handler),
            },
            cx,
        );
        let point = settings.curve.get(self.point).copied();
        let travel_percent = point.map_or(0, |point| {
            curve::percent(f32::from(point.travel) / POINT_SCALE)
        });
        let output_percent = point.map_or(0, |point| {
            curve::percent(f32::from(point.output) / POINT_SCALE)
        });
        let (travel_label, output_label) = match self.point {
            0 => ("Key start dead zone", "Anti-dead zone"),
            point if point == CURVE_POINTS - 1 => ("Key end dead zone", "Maximum output"),
            _ => ("Travel", "Analog value"),
        };
        let travel_text = if self.point == CURVE_POINTS - 1 {
            format!("{travel_label}: {}%", 100 - travel_percent)
        } else {
            format!("{travel_label}: {travel_percent}%")
        };
        let points = TabBar::new("curve-point")
            .segmented()
            .small()
            .selected_index(self.point)
            .on_click(cx.listener(|this, index: &usize, window, cx| {
                this.choose_point(*index, window, cx);
            }))
            .children(POINT_NAMES.map(|name| Tab::new().label(name)));
        section
            .child(presets)
            .child(plot)
            .child(points)
            .child(panel::row(
                SharedString::from(travel_text),
                Some("How deep the key is pressed at this point.".into()),
                div().w(px(240.0)).child(Slider::new(&self.point_travel)),
                cx,
            ))
            .child(panel::row(
                SharedString::from(format!("{output_label}: {output_percent}%")),
                Some("The analog value the controller sends at this point.".into()),
                div().w(px(240.0)).child(Slider::new(&self.point_output)),
                cx,
            ))
    }

    fn joystick_section(&self, cx: &mut Context<Self>) -> Div {
        let settings = self.analog;
        let bias = settings.and_then(|settings| settings.left_stick_up_bias);
        panel::section("Joystick", None, cx)
            .child(panel::row(
                "Use square joystick output",
                Some(
                    "Restrict the joystick movement vector to a square instead of a circle. This \
                     makes the axis input absolute, allowing maximum axis output. Leave this off \
                     to match a standard joystick; it may break compatibility with some games."
                        .into(),
                ),
                Switch::new("square-joystick")
                    .checked(
                        settings.is_some_and(|settings| {
                            settings.normalization == Normalization::Square
                        }),
                    )
                    .disabled(settings.is_none())
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let normalization = if *checked {
                            Normalization::Square
                        } else {
                            Normalization::Circular
                        };
                        this.change_analog(
                            |settings| settings.normalization = normalization,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(panel::row(
                "Snappy joystick",
                Some(
                    "When opposite directions of an axis are both pressed, the deeper one wins \
                     instead of the two cancelling each other out."
                        .into(),
                ),
                Switch::new("snappy-joystick")
                    .checked(settings.is_some_and(|settings| settings.single_direction_axes))
                    .disabled(settings.is_none())
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let snappy = *checked;
                        this.change_analog(
                            |settings| settings.single_direction_axes = snappy,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(panel::row(
                "Enable angle adjustment",
                Some(
                    "When pressing forward and left or right you walk diagonally. Adjust the \
                     slider to move the centre of the diagonal direction of the left stick."
                        .into(),
                ),
                Switch::new("joystick-angle")
                    .checked(bias.is_some())
                    .disabled(settings.is_none())
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        let bias = checked.then_some(DEFAULT_UP_BIAS);
                        this.change_analog(
                            |settings| settings.left_stick_up_bias = bias,
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .child(panel::row(
                SharedString::from(format!(
                    "Adjust the diagonal angle: {}% sideways",
                    bias.unwrap_or(DEFAULT_UP_BIAS)
                )),
                Some("50% walks at 45 degrees; more turns the diagonal towards the side.".into()),
                div()
                    .w(px(240.0))
                    .child(Slider::new(&self.up_bias).disabled(bias.is_none())),
                cx,
            ))
    }

    fn tester_view(&self, keyboard: &Keyboard, window: &Window, cx: &mut Context<Self>) -> Div {
        let running = self.started_with.map(|settings| settings.mode);
        let gamepad_running = running.is_some_and(|mode| mode != Mode::NoGamepad);
        let detection = panel::section(
            "Identify controller",
            Some(
                "The controller sends a fixed pattern instead of the keys (the Xbox controller \
                 points its left stick to the corner of its player number), so you can find it \
                 in a game or in the system's controller settings."
                    .into(),
            ),
            cx,
        )
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(
                    detection_button(self.detecting)
                        .disabled(!gamepad_running)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let enabled = !this.detecting;
                            this.set_detection(enabled, cx);
                        })),
                )
                .child(panel::note(
                    if gamepad_running {
                        "Detection stops when you leave this tab."
                    } else {
                        "The gamepad is off. Choose a controller type and restart the keyboard first."
                    },
                    cx,
                )),
        );
        let curve = self.analog.map(|settings| settings.curve);
        let theme = cx.theme();
        let travel_fill = theme.primary.opacity(0.15);
        let board = KeyboardView::new("gamepad-tester", keyboard, |key| {
            tester_face(
                keyboard,
                key,
                self.binding(key),
                self.key_travel(key),
                curve,
                travel_fill,
            )
        })
        .fit_width(window.viewport_size().width - px(KEYBOARD_MARGIN));
        let mut live = panel::section(
            "Live key travel",
            Some(
                "Press mapped keys to see how deep they are pressed and the analog value the \
                 curve makes of it. The keyboard is read while this tab is open."
                    .into(),
            ),
            cx,
        )
        .child(div().w_full().overflow_hidden().child(board));
        if let Some(curve) = curve {
            let markers = (0..keyboard.key_count())
                .filter(|key| self.binding(*key).is_some_and(controls::is_analog))
                .filter_map(|key| self.key_travel(key))
                .filter(|travel| *travel > 0)
                .map(|travel| {
                    let level = curve::travel_level(travel);
                    (level, curve::output(curve, level))
                })
                .collect();
            live = live.child(chart::chart(
                Chart {
                    curve,
                    selected: None,
                    markers,
                    handler: None,
                },
                cx,
            ));
        }
        v_flex().gap_4().child(detection).child(live)
    }
}

impl Render for GamepadPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_at = Instant::now();
        let page = panel::page(
            "Gamepad",
            "Your keyboard can work just like a controller, sending out analog key signals.",
            cx,
        );
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        let tabs = TabBar::new("gamepad-tabs")
            .segmented()
            .selected_index(
                View::ALL
                    .iter()
                    .position(|view| *view == self.view)
                    .unwrap_or(0),
            )
            .on_click(cx.listener(|this, index: &usize, window, cx| {
                if let Some(view) = View::ALL.get(*index) {
                    this.choose_view(*view, window, cx);
                }
            }))
            .children(View::ALL.map(|view| Tab::new().label(view.title())));
        let content = match self.view {
            View::Setup => self.setup_view(&keyboard, window, cx),
            View::Configuration => self.configuration_view(cx),
            View::Tester => self.tester_view(&keyboard, window, cx),
        };
        page.child(div().child(tabs)).child(content)
    }
}

fn detection_button(detecting: bool) -> Button {
    let button = Button::new("gamepad-detection");
    if detecting {
        button.danger().label("Stop identifying")
    } else {
        button.primary().label("Identify controller")
    }
}

fn key_legend(keyboard: &Keyboard, key: usize) -> SharedString {
    keyboard
        .legends
        .get(key)
        .map_or_else(SharedString::default, |action| {
            SharedString::from(usage::legend(*action).into_owned())
        })
}

fn mapping_face(
    keyboard: &Keyboard,
    key: usize,
    binding: Option<Control>,
    mode: Mode,
    mapped_fill: Hsla,
    unsupported_fill: Hsla,
) -> KeyFace {
    let legend = key_legend(keyboard, key);
    match binding {
        Some(control) => KeyFace {
            legend: usage::control(control).0.into(),
            detail: Some(legend),
            fill: Some(
                if controls::works_in(control, mode) || mode == Mode::NoGamepad {
                    mapped_fill
                } else {
                    unsupported_fill
                },
            ),
            travel: None,
            dimmed: false,
        },
        None => KeyFace {
            legend,
            dimmed: true,
            ..KeyFace::default()
        },
    }
}

fn tester_face(
    keyboard: &Keyboard,
    key: usize,
    binding: Option<Control>,
    travel: Option<u16>,
    curve: Option<curve::Curve>,
    mapped_fill: Hsla,
) -> KeyFace {
    let legend = key_legend(keyboard, key);
    let Some(control) = binding else {
        return KeyFace {
            legend,
            dimmed: true,
            ..KeyFace::default()
        };
    };
    let level = travel.map(curve::travel_level);
    let detail = match (level, curve) {
        (Some(level), Some(curve)) if controls::is_analog(control) => {
            format!("{}%", curve::percent(curve::output(curve, level)))
        }
        (Some(level), _) => format!("{}% travel", curve::percent(level)),
        (None, _) => legend.to_string(),
    };
    KeyFace {
        legend: usage::control(control).0.into(),
        detail: Some(detail.into()),
        fill: Some(mapped_fill),
        travel: level,
        dimmed: false,
    }
}
