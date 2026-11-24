use gpui_kit::{
    AppContext as _, Context, Div, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window,
    component::{
        Disableable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        notification::Notification,
        radio::{Radio, RadioGroup},
        slider::{Slider, SliderEvent, SliderState},
        switch::Switch,
        v_flex,
    },
};
use openwoot_protocol::{
    device::{self, KeyboardFormat, Layout, Speed},
    endpoints::{
        FactoryReset, GetDeviceSettings, GetGlobalSettings, Restart, SetDeviceSettings,
        SetGlobalSettings, SetLayout,
    },
    global,
    lighting::Idle,
};

use crate::{
    panel,
    store::{Scope, Store, StoreEvent},
};

const MAX_IDLE_MINUTES: f32 = 60.0;
const DEFAULT_IDLE: Idle = Idle {
    dim_after_minutes: 5,
    off_after_minutes: 15,
    off_enabled: true,
};

pub(crate) struct SettingsPage {
    store: Entity<Store>,
    device: Option<device::Settings>,
    global: Option<global::Settings>,
    layout: Option<Layout>,
    dim_after: Entity<SliderState>,
    off_after: Entity<SliderState>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let minutes = |cx: &mut Context<Self>| {
            cx.new(|_| {
                SliderState::new()
                    .min(1.0)
                    .max(MAX_IDLE_MINUTES)
                    .step(1.0)
                    .default_value(f32::from(DEFAULT_IDLE.dim_after_minutes))
            })
        };
        let dim_after = minutes(cx);
        let off_after = minutes(cx);
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected | StoreEvent::ProfileChanged | StoreEvent::Failed(_) => {}
            }),
            cx.subscribe(&dim_after, |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Release(value) = event {
                    this.change_idle(|idle| idle.dim_after_minutes = minutes_of(value.end()), cx);
                }
            }),
            cx.subscribe(&off_after, |this, _, event: &SliderEvent, cx| {
                if let SliderEvent::Release(value) = event {
                    this.change_idle(|idle| idle.off_after_minutes = minutes_of(value.end()), cx);
                }
            }),
        ];
        let mut page = Self {
            store,
            device: None,
            global: None,
            layout: None,
            dim_after,
            off_after,
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.layout = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.layout);
        let device = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        let global = self
            .store
            .update(cx, |store, cx| store.fetch::<GetGlobalSettings, _>((), cx));
        cx.spawn_in(window, async move |this, cx| {
            let device = device.await.ok();
            let global = global.await.ok();
            this.update_in(cx, |page, window, cx| {
                page.device = device;
                page.global = global;
                let idle = global
                    .and_then(|global| global.idle)
                    .unwrap_or(DEFAULT_IDLE);
                page.dim_after.update(cx, |slider, cx| {
                    slider.set_value(f32::from(idle.dim_after_minutes), window, cx);
                });
                page.off_after.update(cx, |slider, cx| {
                    slider.set_value(f32::from(idle.off_after_minutes), window, cx);
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change_device(
        &mut self,
        change: impl FnOnce(&mut device::Settings) + 'static,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        cx.spawn(async move |this, cx| {
            let Ok(mut settings) = current.await else {
                return;
            };
            change(&mut settings);
            let Ok(applied) = this.update(cx, |page, cx| {
                page.device = Some(settings);
                cx.notify();
                page.store.update(cx, |store, cx| {
                    store.apply::<SetDeviceSettings>(settings, Scope::Keyboard, cx)
                })
            }) else {
                return;
            };
            applied.await.ok();
        })
        .detach();
    }

    fn change_global(
        &mut self,
        change: impl FnOnce(&mut global::Settings),
        cx: &mut Context<Self>,
    ) {
        let Some(settings) = &mut self.global else {
            return;
        };
        change(settings);
        let settings = *settings;
        self.store
            .update(cx, |store, cx| {
                store.apply::<SetGlobalSettings>(settings, Scope::Keyboard, cx)
            })
            .detach();
        cx.notify();
    }

    fn change_idle(&mut self, change: impl FnOnce(&mut Idle), cx: &mut Context<Self>) {
        self.change_global(
            |settings| {
                if let Some(idle) = &mut settings.idle {
                    change(idle);
                }
            },
            cx,
        );
    }

    fn choose_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        self.layout = Some(layout);
        self.store
            .update(cx, |store, cx| {
                store.apply::<SetLayout>(layout, Scope::Keyboard, cx)
            })
            .detach();
        cx.notify();
    }

    fn restart(&self, window: &mut Window, cx: &mut Context<Self>) {
        let restart = self.store.read(cx).device().call::<Restart>(());
        cx.spawn_in(window, async move |_, cx| {
            let message = match restart.await {
                Ok(()) => Notification::info("The keyboard restarts"),
                Err(error) => Notification::error(error.to_string()),
            };
            cx.update(|window, cx| window.push_notification(message, cx))
                .ok();
        })
        .detach();
    }

    fn confirm_factory_reset(&self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let store = store.clone();
            alert
                .title("Reset the keyboard to factory settings?")
                .description(
                    "Every profile, key binding, lighting effect and setting returns to the \
                     factory defaults. This cannot be undone.",
                )
                .confirm()
                .ok_text("Reset everything")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    store.update(cx, |store, cx| {
                        let reset = store.apply::<FactoryReset>((), Scope::Runtime, cx);
                        cx.spawn(async move |store, cx| {
                            if reset.await.is_ok()
                                && let Ok(reload) = store.update(cx, |store, cx| store.reload(cx))
                            {
                                reload.await;
                            }
                        })
                        .detach();
                    });
                    true
                })
        });
    }

    fn layout_section(&self, cx: &mut Context<Self>) -> Div {
        let layouts: Vec<Layout> = self
            .store
            .read(cx)
            .keyboard()
            .map_or_else(Vec::new, |keyboard| {
                keyboard.capabilities.layouts.iter().copied().collect()
            });
        panel::section(
            "Layout",
            Some(
                "The physical layout of the keycaps. A new layout takes effect after saving and \
                 restarting the keyboard."
                    .into(),
            ),
            cx,
        )
        .child(
            RadioGroup::horizontal("layout")
                .children(
                    layouts
                        .iter()
                        .map(|layout| Radio::new(layout_name(*layout)).label(layout_name(*layout))),
                )
                .selected_index(
                    self.layout
                        .and_then(|current| layouts.iter().position(|layout| *layout == current)),
                )
                .on_change(cx.listener(move |this, index: &usize, _, cx| {
                    if let Some(layout) = layouts.get(*index) {
                        this.choose_layout(*layout, cx);
                    }
                })),
        )
    }

    fn usb_section(&self, cx: &mut Context<Self>) -> Div {
        let device = self.device;
        panel::section(
            "USB",
            Some("These settings take effect the next time the keyboard starts.".into()),
            cx,
        )
        .child(panel::row(
            "NKRO compatibility mode",
            Some(
                "Send the 6-key boot keyboard report instead of the N-key rollover report, for \
                 BIOS screens, KVM switches and consoles that cannot read the latter."
                    .into(),
            ),
            Switch::new("boot-keyboard")
                .checked(device.is_some_and(|settings| settings.keyboard == KeyboardFormat::Boot))
                .disabled(device.is_none())
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    let format = if *checked {
                        KeyboardFormat::Boot
                    } else {
                        KeyboardFormat::NKeyRollover
                    };
                    this.change_device(move |settings| settings.keyboard = format, cx);
                })),
            cx,
        ))
        .child(panel::row(
            "Force full speed",
            Some(
                "Run USB at full speed (1000 Hz polling) instead of high speed (8000 Hz) on \
                 keyboards that support it, for hubs and hosts that misbehave at high speed."
                    .into(),
            ),
            Switch::new("full-speed")
                .checked(device.is_some_and(|settings| settings.speed == Speed::Full))
                .disabled(device.is_none())
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    let speed = if *checked { Speed::Full } else { Speed::High };
                    this.change_device(move |settings| settings.speed = speed, cx);
                })),
            cx,
        ))
    }

    fn behaviour_section(&self, cx: &mut Context<Self>) -> Div {
        let global = self.global;
        panel::section("Behaviour", None, cx)
            .child(panel::row(
                "Invert Num Lock indicator",
                Some("Light the Num Lock indicator while Num Lock is off, for macOS which never turns it on.".into()),
                Switch::new("num-lock-inverted")
                    .checked(global.is_some_and(|settings| settings.num_lock_inverted))
                    .disabled(global.is_none())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        let inverted = *checked;
                        this.change_global(|settings| settings.num_lock_inverted = inverted, cx);
                    })),
                cx,
            ))
            .child(panel::row(
                "Debounce filter",
                Some(
                    "Ignore a key that bounces back right after it was released, at the cost of a \
                     little latency on the next press."
                        .into(),
                ),
                Switch::new("release-rebound-filter")
                    .checked(global.is_some_and(|settings| settings.release_rebound_filter))
                    .disabled(global.is_none())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        let filtered = *checked;
                        this.change_global(|settings| settings.release_rebound_filter = filtered, cx);
                    })),
                cx,
            ))
    }

    fn idle_section(&self, cx: &mut Context<Self>) -> Div {
        let global = self.global;
        let idle = global.and_then(|global| global.idle);
        panel::section(
            "Idle lighting",
            Some("Dim and turn off the LEDs while no key is touched.".into()),
            cx,
        )
        .child(panel::row(
            "Dim when idle",
            None,
            Switch::new("idle-enabled")
                .checked(idle.is_some())
                .disabled(global.is_none())
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    let idle = checked.then_some(DEFAULT_IDLE);
                    this.change_global(|settings| settings.idle = idle, cx);
                })),
            cx,
        ))
        .child(panel::row(
            SharedString::from(format!(
                "Dim after {} min",
                idle.map_or(DEFAULT_IDLE.dim_after_minutes, |idle| idle
                    .dim_after_minutes)
            )),
            None,
            Slider::new(&self.dim_after).disabled(idle.is_none()),
            cx,
        ))
        .child(panel::row(
            "Turn off when idle",
            None,
            Switch::new("idle-off")
                .checked(idle.is_some_and(|idle| idle.off_enabled))
                .disabled(idle.is_none())
                .on_change(cx.listener(|this, checked: &bool, _, cx| {
                    let enabled = *checked;
                    this.change_idle(|idle| idle.off_enabled = enabled, cx);
                })),
            cx,
        ))
        .child(panel::row(
            SharedString::from(format!(
                "Turn off after {} min",
                idle.map_or(DEFAULT_IDLE.off_after_minutes, |idle| idle
                    .off_after_minutes)
            )),
            None,
            Slider::new(&self.off_after).disabled(!idle.is_some_and(|idle| idle.off_enabled)),
            cx,
        ))
    }

    fn maintenance_section(cx: &mut Context<Self>) -> Div {
        panel::section("Maintenance", None, cx)
            .child(panel::row(
                "Restart",
                Some("Restart the keyboard; unsaved changes are lost.".into()),
                Button::new("restart")
                    .outline()
                    .label("Restart keyboard")
                    .on_click(cx.listener(|this, _, window, cx| this.restart(window, cx))),
                cx,
            ))
            .child(panel::row(
                "Factory reset",
                Some("Return every profile and setting to the factory defaults.".into()),
                Button::new("factory-reset")
                    .danger()
                    .label("Factory reset")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_factory_reset(window, cx)),
                    ),
                cx,
            ))
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = v_flex()
            .gap_4()
            .max_w_full()
            .child(self.layout_section(cx))
            .child(self.usb_section(cx))
            .child(self.behaviour_section(cx))
            .child(self.idle_section(cx))
            .child(Self::maintenance_section(cx));
        panel::page(
            "Keyboard settings",
            "Settings that apply to the whole keyboard, independent of the profile.",
            cx,
        )
        .child(sections)
    }
}

const fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::Ansi => "ANSI",
        Layout::Iso => "ISO",
        Layout::Jis => "JIS",
    }
}

fn minutes_of(value: f32) -> u8 {
    let clamped = value.round().clamp(1.0, MAX_IDLE_MINUTES);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 1..=60"
    )]
    let minutes = clamped as u8;
    minutes
}
