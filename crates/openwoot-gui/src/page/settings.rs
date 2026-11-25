mod choice;
mod layout;
mod sheet;
mod sleep;

use gpui_kit::{
    AppContext as _, Context, Div, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Subscription, Window,
    component::{
        Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        notification::Notification,
        slider::{SliderEvent, SliderState},
    },
    prelude::FluentBuilder as _,
};
use openwoot_protocol::{
    device::{self, KeyboardFormat, Layout, LayoutState, Speed},
    endpoints::{
        FactoryReset, GetDeviceSettings, GetGlobalSettings, GetLayout, Restart, SetDeviceSettings,
        SetGlobalSettings, SetLayout,
    },
    global,
};

use self::sleep::Sleep;
pub(super) use self::{
    layout::name as layout_name,
    sheet::{Tone, callout, entry, sheet},
};
use super::Page;
use crate::{
    panel,
    store::{Scope, Store, StoreEvent},
    switch::Switch,
};

const POLLING_RATES: [(Speed, &str); 2] = [(Speed::High, "8000 Hz"), (Speed::Full, "1000 Hz")];
const NUM_LOCK: [(bool, &str); 2] = [
    (false, "Regular: Input numbers when the indicator is on"),
    (true, "Inverted: Input numbers when the indicator is off"),
];

pub(crate) struct SettingsPage {
    store: Entity<Store>,
    device: Option<device::Settings>,
    started_with: Option<device::Settings>,
    global: Option<global::Settings>,
    layout: Option<LayoutState>,
    sleep: Sleep,
    dim_after: Entity<SliderState>,
    off_after: Entity<SliderState>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dim_after = cx.new(|_| sleep::slider_state());
        let off_after = cx.new(|_| sleep::slider_state());
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected => {
                    this.started_with = None;
                    this.load(window, cx);
                }
                StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected | StoreEvent::ProfileChanged | StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &dim_after,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(value) => {
                        let sleep = this.sleep.with_dim(sleep::stop_of(value.end()));
                        this.move_sleep(sleep, window, cx);
                    }
                    SliderEvent::Release(_) => this.store_sleep(cx),
                },
            ),
            cx.subscribe_in(
                &off_after,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(value) => {
                        let sleep = this.sleep.with_off(sleep::stop_of(value.end()));
                        this.move_sleep(sleep, window, cx);
                    }
                    SliderEvent::Release(_) => this.store_sleep(cx),
                },
            ),
        ];
        let mut page = Self {
            store,
            device: None,
            started_with: None,
            global: None,
            layout: None,
            sleep: Sleep::of(None),
            dim_after,
            off_after,
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (device, global, layout) = self.store.update(cx, |store, cx| {
            (
                store.fetch::<GetDeviceSettings, _>((), cx),
                store.fetch::<GetGlobalSettings, _>((), cx),
                store.fetch::<GetLayout, _>((), cx),
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let device = device.await.ok();
            let global = global.await.ok();
            let layout = layout.await.ok();
            this.update_in(cx, |page, window, cx| {
                page.device = device;
                if page.started_with.is_none() {
                    page.started_with = device;
                }
                page.global = global;
                page.layout = layout;
                page.sleep = Sleep::of(global.and_then(|global| global.idle));
                page.sync_sleep(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change_device(
        &mut self,
        change: impl Fn(&mut device::Settings) + 'static,
        cx: &mut Context<Self>,
    ) {
        if let Some(settings) = &mut self.device {
            change(settings);
        }
        cx.notify();
        let current = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        cx.spawn(async move |this, cx| {
            let Ok(stored) = current.await else {
                return;
            };
            let mut settings = stored;
            change(&mut settings);
            let Ok(applied) = this.update(cx, |page, cx| {
                page.store.update(cx, |store, cx| {
                    store.change::<SetDeviceSettings>(&stored, settings, Scope::Keyboard, cx)
                })
            }) else {
                return;
            };
            let written = applied.await.is_ok();
            this.update(cx, |page, cx| {
                page.device = Some(if written { settings } else { stored });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change_global(
        &mut self,
        change: impl FnOnce(&mut global::Settings),
        cx: &mut Context<Self>,
    ) {
        let Some(previous) = self.global else {
            return;
        };
        let mut settings = previous;
        change(&mut settings);
        if settings == previous {
            return;
        }
        self.global = Some(settings);
        cx.notify();
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetGlobalSettings>(&previous, settings, Scope::Keyboard, cx)
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_err() {
                this.update(cx, |page, cx| {
                    if page.global == Some(settings) {
                        page.global = Some(previous);
                        page.sleep = Sleep::of(previous.idle);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn sync_sleep(&self, window: &mut Window, cx: &mut Context<Self>) {
        let dim = sleep::position(self.sleep.dim);
        let off = sleep::position(self.sleep.off);
        self.dim_after
            .update(cx, |slider, cx| slider.set_value(dim, window, cx));
        self.off_after
            .update(cx, |slider, cx| slider.set_value(off, window, cx));
    }

    fn move_sleep(&mut self, sleep: Sleep, window: &mut Window, cx: &mut Context<Self>) {
        if self.sleep != sleep {
            self.sleep = sleep;
            self.sync_sleep(window, cx);
            cx.notify();
        }
    }

    fn store_sleep(&mut self, cx: &mut Context<Self>) {
        let idle = self.sleep.idle();
        if self.global.is_some_and(|global| global.idle != idle) {
            self.change_global(move |settings| settings.idle = idle, cx);
        }
    }

    fn pick_sleep(&mut self, sleep: Sleep, window: &mut Window, cx: &mut Context<Self>) {
        self.move_sleep(sleep, window, cx);
        self.store_sleep(cx);
    }

    fn choose_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        let Some(previous) = self.layout else {
            return;
        };
        if previous.chosen == layout {
            return;
        }
        self.layout = Some(LayoutState {
            chosen: layout,
            ..previous
        });
        cx.notify();
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetLayout>(&previous.chosen, layout, Scope::Keyboard, cx)
        });
        cx.spawn(async move |this, cx| {
            if applied.await.is_err() {
                this.update(cx, |page, cx| {
                    if page.layout.is_some_and(|state| state.chosen == layout) {
                        page.layout = Some(previous);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn changed_since_start<T: PartialEq>(&self, field: impl Fn(&device::Settings) -> T) -> bool {
        matches!(
            (self.device, self.started_with),
            (Some(now), Some(started)) if field(&now) != field(&started)
        )
    }

    fn layout_pending(&self) -> bool {
        self.layout
            .is_some_and(|state| state.active != state.chosen)
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

    fn restart(&self, window: &mut Window, cx: &mut Context<Self>) {
        let restart = self.store.read(cx).device().call::<Restart>(());
        cx.spawn_in(window, async move |_, cx| {
            if let Err(error) = restart.await {
                let message = Notification::error(error.to_string());
                cx.update(|window, cx| window.push_notification(message, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn confirm_factory_reset(&self, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let store = store.clone();
            alert
                .title("Reset all keyboard settings?")
                .description("Every profile and setting returns to the factory state.")
                .confirm()
                .ok_text("Reset")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, _, cx| {
                    store.update(cx, |store, cx| {
                        let reset = store.apply::<FactoryReset>((), Scope::Runtime, cx);
                        cx.spawn(async move |store, cx| {
                            if reset.await.is_ok()
                                && let Ok(reload) = store.update(cx, |store, cx| {
                                    store.forget_changes(cx);
                                    store.reload(cx)
                                })
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

    fn restart_callout(id: &'static str, cx: &mut Context<Self>) -> Div {
        callout(Tone::Info, "Applies after a restart.", cx).child(
            Button::new(id)
                .primary()
                .small()
                .label("Save and restart")
                .on_click(cx.listener(|this, _, window, cx| this.save_and_restart(window, cx))),
        )
    }

    fn compatibility_section(&self, cx: &mut Context<Self>) -> Div {
        let boot = self
            .device
            .map(|settings| settings.keyboard == KeyboardFormat::Boot);
        entry(
            "Compatibility mode (disable NKRO)",
            Some(
                "Reports six keys at a time, for BIOS screens, consoles and old computers.".into(),
            ),
            Some(
                Switch::new("compatibility-mode")
                    .checked(boot.unwrap_or(false))
                    .disabled(boot.is_none())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        let format = if *checked {
                            KeyboardFormat::Boot
                        } else {
                            KeyboardFormat::NKeyRollover
                        };
                        this.change_device(move |settings| settings.keyboard = format, cx);
                    }))
                    .into_any_element(),
            ),
            cx,
        )
        .when(
            self.changed_since_start(|settings| settings.keyboard)
                && !self.changed_since_start(|settings| settings.speed),
            |this| this.child(Self::restart_callout("restart-compatibility", cx)),
        )
    }

    fn polling_section(&self, cx: &mut Context<Self>) -> Div {
        entry(
            "Polling rate",
            Some(
                "How often the keyboard reports to the computer; lower it if the connection is \
                 unstable."
                    .into(),
            ),
            None,
            cx,
        )
        .child(choice::choices(
            "polling-rate",
            &POLLING_RATES,
            self.device.map(|settings| settings.speed),
            cx.listener(|this, speed: &Speed, _, cx| {
                let speed = *speed;
                this.change_device(move |settings| settings.speed = speed, cx);
            }),
            cx,
        ))
        .when(
            self.changed_since_start(|settings| settings.speed),
            |this| this.child(Self::restart_callout("restart-polling-rate", cx)),
        )
    }

    fn num_lock_section(&self, cx: &mut Context<Self>) -> Div {
        entry(
            "Num Lock indicator",
            cfg!(target_os = "macos").then(|| {
                SharedString::from("macOS has no Num Lock; choose regular to hide the indicator.")
            }),
            None,
            cx,
        )
        .child(choice::choices(
            "num-lock",
            &NUM_LOCK,
            self.global.map(|settings| settings.num_lock_inverted),
            cx.listener(|this, inverted: &bool, _, cx| {
                let inverted = *inverted;
                this.change_global(|settings| settings.num_lock_inverted = inverted, cx);
            }),
            cx,
        ))
    }

    fn sleep_section(&self, cx: &mut Context<Self>) -> Div {
        let disabled = self.global.is_none();
        entry(
            "RGB sleep mode",
            Some("Dims and then turns off the lighting while no key is pressed.".into()),
            None,
            cx,
        )
        .child(panel::field(
            "Turn off after",
            sleep::stepped(
                "turn-off-after",
                &self.off_after,
                self.sleep.off,
                disabled,
                cx.listener(|this, stop: &u8, window, cx| {
                    let sleep = this.sleep.with_off(*stop);
                    this.pick_sleep(sleep, window, cx);
                }),
                cx,
            ),
            cx,
        ))
        .child(panel::field(
            "Dim after",
            sleep::stepped(
                "dim-after",
                &self.dim_after,
                self.sleep.dim,
                disabled,
                cx.listener(|this, stop: &u8, window, cx| {
                    let sleep = this.sleep.with_dim(*stop);
                    this.pick_sleep(sleep, window, cx);
                }),
                cx,
            ),
            cx,
        ))
    }

    fn debounce_section(&self, cx: &mut Context<Self>) -> Div {
        let filtered = self.global.map(|settings| settings.release_rebound_filter);
        entry(
            "Debounce filter",
            Some("Ignores keys that bounce back down after a release; adds no latency.".into()),
            Some(
                Switch::new("debounce-filter")
                    .checked(filtered.unwrap_or(false))
                    .disabled(filtered.is_none())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        let filtered = *checked;
                        this.change_global(
                            |settings| settings.release_rebound_filter = filtered,
                            cx,
                        );
                    }))
                    .into_any_element(),
            ),
            cx,
        )
    }

    fn layout_section(&self, layouts: &[Layout], cx: &mut Context<Self>) -> Div {
        entry(
            "Layout",
            Some("Match the physical layout of your keyboard.".into()),
            None,
            cx,
        )
        .child(layout::cards(
            layouts,
            self.layout.map(|state| state.chosen),
            cx.listener(|this, layout: &Layout, _, cx| this.choose_layout(*layout, cx)),
            cx,
        ))
        .when(self.layout_pending(), |this| {
            this.child(Self::restart_callout("restart-layout", cx))
        })
    }

    fn restart_section(cx: &mut Context<Self>) -> Div {
        entry(
            "Restart keyboard",
            Some("Starts the firmware again; unsaved changes are lost.".into()),
            None,
            cx,
        )
        .child(
            h_flex().child(
                Button::new("restart")
                    .outline()
                    .label("Restart")
                    .on_click(cx.listener(|this, _, window, cx| this.restart(window, cx))),
            ),
        )
    }

    fn reset_section(cx: &mut Context<Self>) -> Div {
        entry(
            "Reset all keyboard settings",
            Some("Restores every profile and setting to the factory state.".into()),
            None,
            cx,
        )
        .child(
            h_flex().child(
                Button::new("factory-reset")
                    .danger()
                    .label("Reset all keyboard settings")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.confirm_factory_reset(window, cx)),
                    ),
            ),
        )
        .child(callout(
            Tone::Danger,
            "This cannot be undone; every profile and setting is lost.",
            cx,
        ))
    }
}

impl Render for SettingsPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layouts: Vec<Layout> = self
            .store
            .read(cx)
            .keyboard()
            .map_or_else(Vec::new, |keyboard| {
                keyboard.capabilities.layouts.iter().copied().collect()
            });
        let mut sections = vec![
            self.compatibility_section(cx),
            self.polling_section(cx),
            self.num_lock_section(cx),
            self.sleep_section(cx),
            self.debounce_section(cx),
        ];
        if layouts.len() > 1 {
            sections.push(self.layout_section(&layouts, cx));
        }
        sections.push(Self::restart_section(cx));
        sections.push(Self::reset_section(cx));
        panel::page().child(sheet(Page::Settings.title(), sections, cx))
    }
}
