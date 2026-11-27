mod choice;
mod layout;
mod sheet;
mod sleep;

use gpui_kit::{
    Context, Div, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window,
    component::{
        ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, notification::Notification,
        spinner::Spinner, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    device::{self, KeyboardFormat, Layout, LayoutState, Speed},
    endpoints::{
        FactoryReset, GetDeviceSettings, GetGlobalSettings, GetLayout, Restart, SetDeviceSettings,
        SetGlobalSettings, SetLayout,
    },
    global,
};

use self::sleep::{Sleep, Step};
pub(super) use self::{
    layout::name as layout_name,
    sheet::{Tone, body_text, callout, emphasised, entry, heading, sheet},
};
use super::Page;
use crate::{
    panel::{self, ActionButton},
    store::{Scope, Store, StoreEvent},
    switch::Switch,
};

const POLLING_RATES: [(Speed, &str); 2] = [(Speed::High, "8000 Hz"), (Speed::Full, "1000 Hz")];
const SLEEP_TOP: f32 = 16.0;
const SLEEP_GAP: f32 = 24.0;
const NUM_LOCK: [(bool, &str); 2] = [
    (false, "Regular: Input numbers when the indicator is on"),
    (true, "Inverted: Input numbers when the indicator is off"),
];
const RESTART_FAILED: &str = "Could not restart the keyboard.";
const RESETTING: &str = "Resetting the keyboard";
const RESET_WAIT: &str = "Erasing the settings takes a few seconds. Keep the keyboard connected.";
const RESET_DONE: &str = "The keyboard is back to its factory settings";

pub(crate) struct SettingsPage {
    store: Entity<Store>,
    device: Option<device::SettingsState>,
    global: Option<global::Settings>,
    layout: Option<LayoutState>,
    sleep: Sleep,
    resetting: bool,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected | StoreEvent::ProfileChanged | StoreEvent::Failed(_) => {}
            }),
        ];
        let mut page = Self {
            store,
            device: None,
            global: None,
            layout: None,
            sleep: Sleep::of(None),
            resetting: false,
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
            this.update(cx, |page, cx| {
                page.device = device;
                page.global = global;
                page.layout = layout;
                page.sleep = Sleep::of(global.and_then(|global| global.idle));
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
        if let Some(state) = &mut self.device {
            change(&mut state.chosen);
        }
        cx.notify();
        let current = self
            .store
            .update(cx, |store, cx| store.fetch::<GetDeviceSettings, _>((), cx));
        cx.spawn(async move |this, cx| {
            let Ok(stored) = current.await else {
                return;
            };
            let mut settings = stored.chosen;
            change(&mut settings);
            let Ok(applied) = this.update(cx, |page, cx| {
                page.store.update(cx, |store, cx| {
                    store.change::<SetDeviceSettings>(&stored.chosen, settings, Scope::Keyboard, cx)
                })
            }) else {
                return;
            };
            let written = applied.await.is_ok();
            this.update(cx, |page, cx| {
                page.device = Some(device::SettingsState {
                    chosen: if written { settings } else { stored.chosen },
                    ..stored
                });
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

    fn move_sleep(&mut self, sleep: Sleep, cx: &mut Context<Self>) {
        if self.sleep != sleep {
            self.sleep = sleep;
            cx.notify();
        }
    }

    fn store_sleep(&mut self, cx: &mut Context<Self>) {
        let idle = self.sleep.idle();
        if self.global.is_some_and(|global| global.idle != idle) {
            self.change_global(move |settings| settings.idle = idle, cx);
        }
    }

    fn step_sleep(&mut self, step: Step, change: fn(Sleep, u8) -> Sleep, cx: &mut Context<Self>) {
        match step {
            Step::Move(index) => self.move_sleep(change(self.sleep, index), cx),
            Step::Release => self.store_sleep(cx),
        }
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

    fn pending<T: PartialEq>(&self, field: impl Fn(&device::Settings) -> T) -> bool {
        self.device
            .is_some_and(|state| restart_pending(&state, field))
    }

    fn limited_to_full_speed(&self) -> bool {
        self.device.is_some_and(|state| runs_at_full_speed(&state))
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
            if device.call::<Restart>(()).await.is_err() {
                let message = Notification::error(RESTART_FAILED);
                cx.update(|window, cx| window.push_notification(message, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn restart(&self, window: &mut Window, cx: &mut Context<Self>) {
        let restart = self.store.read(cx).device().call::<Restart>(());
        cx.spawn_in(window, async move |_, cx| {
            if restart.await.is_err() {
                let message = Notification::error(RESTART_FAILED);
                cx.update(|window, cx| window.push_notification(message, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn confirm_factory_reset(window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        panel::confirm(
            panel::Confirmation::new(
                "Are you sure you want to reset?",
                "This will reset all keyboard settings.",
                "Reset",
            )
            .danger(),
            move |window, cx| {
                page.update(cx, |page, cx| page.factory_reset(window, cx))
                    .ok();
            },
            window,
            cx,
        );
    }

    fn factory_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.resetting {
            return;
        }
        self.resetting = true;
        cx.notify();
        let reset = self.store.update(cx, |store, cx| {
            store.apply::<FactoryReset>((), Scope::Runtime, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let done = reset.await.is_ok();
            if done
                && let Ok(reload) = this.update(cx, |page, cx| {
                    page.store.update(cx, |store, cx| {
                        store.forget_changes(cx);
                        store.reload(cx)
                    })
                })
            {
                reload.await;
            }
            this.update_in(cx, |page, window, cx| {
                page.resetting = false;
                if done {
                    window.push_notification(Notification::success(RESET_DONE), cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn restart_callout(id: &'static str, cx: &mut Context<Self>) -> Div {
        callout(Tone::Info, "Applies after a restart.", cx).child(
            ActionButton::new(id, "Save and restart")
                .primary()
                .on_click(cx.listener(|this, _, window, cx| this.save_and_restart(window, cx))),
        )
    }

    fn compatibility_section(&self, cx: &mut Context<Self>) -> Div {
        let boot = self
            .device
            .map(|state| state.chosen.keyboard == KeyboardFormat::Boot);
        entry(
            "Compatibility Mode (Disable NKRO)",
            Some(
                "Turn it on if you have issues with your BIOS or (legacy) devices like a \
                 Playstation or Xbox."
                    .into(),
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
            self.pending(|settings| settings.keyboard) && !self.pending(|settings| settings.speed),
            |this| this.child(Self::restart_callout("restart-compatibility", cx)),
        )
    }

    fn polling_section(&self, cx: &mut Context<Self>) -> Div {
        entry(
            "Polling Rate",
            Some(
                "The polling rate determines how often the keyboard can report to the computer."
                    .into(),
            ),
            None,
            cx,
        )
        .child(choice::choices(
            "polling-rate",
            &POLLING_RATES,
            self.device.map(|state| state.chosen.speed),
            cx.listener(|this, speed: &Speed, _, cx| {
                let speed = *speed;
                this.change_device(move |settings| settings.speed = speed, cx);
            }),
            cx,
        ))
        .map(|this| {
            if self.pending(|settings| settings.speed) {
                this.child(Self::restart_callout("restart-polling-rate", cx))
            } else if self.limited_to_full_speed() {
                this.child(callout(
                    Tone::Info,
                    "The USB connection limits it to 1000 Hz.",
                    cx,
                ))
            } else {
                this
            }
        })
    }

    fn num_lock_section(&self, cx: &mut Context<Self>) -> Div {
        entry(
            "Num Lock indicator behaviour",
            Some("On MacOS, select regular behavior to hide the indicator.".into()),
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
            "RGB Sleep Mode",
            Some(
                "Minutes of inactivity before the keyboard dims the RGB and then turns it off."
                    .into(),
            ),
            None,
            cx,
        )
        .child(
            v_flex()
                .w_full()
                .pt(px(SLEEP_TOP))
                .gap(px(SLEEP_GAP))
                .child(sleep::stepped(
                    "turn-off-after",
                    "TURN OFF AFTER",
                    self.sleep.off,
                    disabled,
                    cx.listener(|this, step: &Step, _, cx| {
                        this.step_sleep(*step, Sleep::with_off, cx);
                    }),
                    cx,
                ))
                .child(sleep::stepped(
                    "dim-after",
                    "DIM AFTER",
                    self.sleep.dim,
                    disabled,
                    cx.listener(|this, step: &Step, _, cx| {
                        this.step_sleep(*step, Sleep::with_dim, cx);
                    }),
                    cx,
                )),
        )
    }

    fn debounce_section(&self, cx: &mut Context<Self>) -> Div {
        let filtered = self.global.map(|settings| settings.release_rebound_filter);
        entry(
            "Debounce Filter",
            Some("Ignores unwanted keypresses from keys bouncing back down when released.".into()),
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
            Some("Restarts the keyboard and loses any unsaved changes.".into()),
            None,
            cx,
        )
        .child(
            h_flex().child(
                ActionButton::new("restart", "Restart")
                    .on_click(cx.listener(|this, _, window, cx| this.restart(window, cx))),
            ),
        )
    }

    fn reset_section(&self, cx: &mut Context<Self>) -> Div {
        let section = entry(
            "Reset all keyboard settings",
            Some("Resetting all keyboard settings will restore the keyboard to its factory settings.".into()),
            None,
            cx,
        );
        if self.resetting {
            let theme = cx.theme();
            return section
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_color(theme.group_box_foreground)
                        .child(Spinner::new().small())
                        .child(div().text_sm().child(RESETTING)),
                )
                .child(callout(Tone::Info, RESET_WAIT, cx));
        }
        section
            .child(
                h_flex().child(
                    ActionButton::new("factory-reset", "Reset all Keyboard Settings")
                        .danger()
                        .on_click(
                            cx.listener(|_, _, window, cx| Self::confirm_factory_reset(window, cx)),
                        ),
                ),
            )
            .child(callout(Tone::Danger, "This action cannot be reversed.", cx))
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
        sections.push(self.reset_section(cx));
        sheet("settings-sheet", Page::Settings.title(), sections, cx)
    }
}

fn restart_pending<T: PartialEq>(
    state: &device::SettingsState,
    field: impl Fn(&device::Settings) -> T,
) -> bool {
    field(&state.active) != field(&state.chosen)
}

fn runs_at_full_speed(state: &device::SettingsState) -> bool {
    state.active.speed == Speed::High
        && state.chosen.speed == Speed::High
        && state.negotiated == Some(Speed::Full)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(active: Speed, chosen: Speed, negotiated: Option<Speed>) -> device::SettingsState {
        device::SettingsState {
            active: device::Settings {
                speed: active,
                ..device::Settings::default()
            },
            chosen: device::Settings {
                speed: chosen,
                ..device::Settings::default()
            },
            negotiated,
        }
    }

    #[test]
    fn a_chosen_rate_waits_for_a_restart_until_it_runs() {
        let waiting = state(Speed::High, Speed::Full, Some(Speed::High));
        assert!(restart_pending(&waiting, |settings| settings.speed));
        assert!(!restart_pending(&waiting, |settings| settings.keyboard));
        let running = state(Speed::Full, Speed::Full, Some(Speed::Full));
        assert!(!restart_pending(&running, |settings| settings.speed));
    }

    #[test]
    fn a_full_speed_port_caps_the_high_rate() {
        assert!(runs_at_full_speed(&state(
            Speed::High,
            Speed::High,
            Some(Speed::Full)
        )));
        assert!(!runs_at_full_speed(&state(
            Speed::High,
            Speed::High,
            Some(Speed::High)
        )));
        assert!(!runs_at_full_speed(&state(Speed::Full, Speed::Full, None)));
        assert!(!runs_at_full_speed(&state(
            Speed::High,
            Speed::Full,
            Some(Speed::Full)
        )));
    }
}
