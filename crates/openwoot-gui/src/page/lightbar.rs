mod bar;
mod condition_editor;
mod entry;
mod entry_editor;
mod entry_list;
mod host;
mod number;
mod preset;
mod preview;
mod swatch;

use std::{mem, time::Duration};

use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window,
    component::{
        ActiveTheme as _,
        color_picker::{ColorPickerEvent, ColorPickerState},
        input::{InputEvent, InputState},
        slider::{SliderEvent, SliderState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    endpoints::{GetGlobalLightbar, GetHostSignals, GetLightbar, SetGlobalLightbar, SetLightbar},
    lightbar::{self, Entries, Entry, HostSignal, HostSignalIds},
    profile::{Change, Slot},
};

use self::{entry::Matrix, number::Field, swatch::Swatch};
use super::{
    Page,
    lighting::{hsla_of, level_of, opaque_of, percent_of},
};
use crate::{
    panel,
    store::{Scope, Store, StoreEvent},
};

const SEND_DELAY: Duration = Duration::from_millis(150);

pub(crate) struct LightbarPage {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<lightbar::Settings>,
    global: Option<Entries>,
    chosen: Option<Chosen>,
    led: u8,
    host_ids: Option<HostSignalIds>,
    host_sent: Vec<HostSignal>,
    problem: Option<SharedString>,
    brightness: Entity<SliderState>,
    range: Option<RangeSlider>,
    swatches: Vec<(Swatch, Entity<ColorPickerState>)>,
    fields: Vec<(Field, Entity<InputState>)>,
    generation: u64,
    pending: Pending,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Profile,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chosen {
    owner: Owner,
    index: usize,
}

#[derive(Default)]
struct Pending {
    profile: bool,
    global: bool,
    timer: Option<Task<()>>,
}

struct RangeSlider {
    state: Entity<SliderState>,
    _subscription: Subscription,
}

impl LightbarPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let brightness = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(100.0)
                .step(1.0)
                .default_value(100.0)
        });
        let swatches: Vec<(Swatch, Entity<ColorPickerState>)> = Swatch::ALL
            .into_iter()
            .map(|swatch| (swatch, cx.new(|cx| ColorPickerState::new(window, cx))))
            .collect();
        let fields: Vec<(Field, Entity<InputState>)> = Field::all()
            .into_iter()
            .map(|field| {
                let state = cx.new(|cx| {
                    InputState::new(window, cx)
                        .step(field.step())
                        .placeholder(field.placeholder())
                });
                (field, state)
            })
            .collect();
        let subscriptions =
            Self::subscriptions(&store, &brightness, &swatches, &fields, window, cx);
        let mut page = Self {
            store,
            profile: None,
            settings: None,
            global: None,
            chosen: None,
            led: 0,
            host_ids: None,
            host_sent: Vec::new(),
            problem: None,
            brightness,
            range: None,
            swatches,
            fields,
            generation: 0,
            pending: Pending::default(),
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn subscriptions(
        store: &Entity<Store>,
        brightness: &Entity<SliderState>,
        swatches: &[(Swatch, Entity<ColorPickerState>)],
        fields: &[(Field, Entity<InputState>)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        let mut subscriptions = vec![
            cx.subscribe_in(store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::Reloaded | StoreEvent::ProfileChanged => {
                    this.load(window, cx);
                }
                StoreEvent::Disconnected => this.forget(cx),
                StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                brightness,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
                        let level = level_of(value.end());
                        this.edit_settings(
                            |settings| settings.brightness = Some(level),
                            window,
                            cx,
                        );
                    }
                },
            ),
        ];
        for (swatch, picker) in swatches {
            let swatch = *swatch;
            subscriptions.push(cx.subscribe_in(
                picker,
                window,
                move |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = opaque_of(*color);
                        let (led, leds) = (this.led, this.leds(cx));
                        this.edit_chosen(
                            |entry| swatch.set(&mut entry.effect, led, leds, color),
                            window,
                            cx,
                        );
                    }
                },
            ));
        }
        for (field, state) in fields {
            let field = *field;
            subscriptions.push(cx.subscribe_in(
                state,
                window,
                move |this, state, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let text = state.read(cx).value();
                        this.field_changed(field, &text, window, cx);
                    }
                },
            ));
        }
        subscriptions
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(window, cx);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let keyboard = self.store.read(cx).keyboard();
        let host_signals = keyboard.is_some_and(|keyboard| keyboard.capabilities.host_signals);
        let leds = keyboard.map_or(0, |keyboard| keyboard.capabilities.lightbar_leds);
        let has_bar = leds > 0;
        self.prepare_range(leds, window, cx);
        self.profile = self.store.read(cx).active();
        self.settings = None;
        self.global = None;
        self.chosen = None;
        self.host_ids = None;
        self.problem = None;
        cx.notify();
        let Some(profile) = self.profile.filter(|_| has_bar) else {
            return;
        };
        let settings = self
            .store
            .update(cx, |store, cx| store.fetch::<GetLightbar, _>(profile, cx));
        let global = self
            .store
            .update(cx, |store, cx| store.fetch::<GetGlobalLightbar, _>((), cx));
        let ids = host_signals.then(|| {
            self.store
                .update(cx, |store, cx| store.fetch::<GetHostSignals, _>((), cx))
        });
        cx.spawn_in(window, async move |this, cx| {
            let settings = settings.await.ok();
            let global = global.await.ok();
            let ids = match ids {
                Some(ids) => ids.await.ok(),
                None => None,
            };
            this.update_in(cx, |page, window, cx| {
                if page.generation != generation {
                    return;
                }
                page.chosen = first_entry(settings.as_ref(), global.as_ref());
                page.settings = settings;
                page.global = global;
                page.host_ids = ids;
                page.led = 0;
                page.sync_controls(window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = Pending::default();
        self.profile = None;
        self.settings = None;
        self.global = None;
        self.chosen = None;
        self.host_ids = None;
        self.problem = None;
        cx.notify();
    }

    fn prepare_range(&mut self, leds: u8, window: &mut Window, cx: &mut Context<Self>) {
        let last = leds.saturating_sub(1);
        if last == 0 {
            self.range = None;
            return;
        }
        let maximum = f32::from(last);
        if self
            .range
            .as_ref()
            .is_some_and(|range| (range.state.read(cx).max_value() - maximum).abs() < f32::EPSILON)
        {
            return;
        }
        let state = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(maximum)
                .step(1.0)
                .default_value((0.0, maximum))
        });
        let subscription = cx.subscribe_in(
            &state,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) => {
                    let (first, last) = (led_of(value.start()), led_of(value.end()));
                    this.edit_range(
                        move |range| {
                            range.first = first;
                            range.last = last;
                        },
                        window,
                        cx,
                    );
                }
            },
        );
        self.range = Some(RangeSlider {
            state,
            _subscription: subscription,
        });
    }

    fn capabilities(&self, cx: &Context<Self>) -> Option<Capabilities> {
        self.store
            .read(cx)
            .keyboard()
            .map(|keyboard| keyboard.capabilities.clone())
    }

    fn leds(&self, cx: &Context<Self>) -> u8 {
        self.store
            .read(cx)
            .keyboard()
            .map_or(0, |keyboard| keyboard.capabilities.lightbar_leds)
    }

    fn matrix(&self, cx: &Context<Self>) -> Matrix {
        self.store.read(cx).keyboard().map_or(
            Matrix {
                leds: 0,
                rows: 0,
                columns: 0,
            },
            |keyboard| Matrix {
                leds: keyboard.capabilities.lightbar_leds,
                rows: keyboard.capabilities.key_rows,
                columns: keyboard.capabilities.key_columns,
            },
        )
    }

    fn entries(&self, owner: Owner) -> Option<&Entries> {
        match owner {
            Owner::Profile => self.settings.as_ref().map(|settings| &settings.entries),
            Owner::Global => self.global.as_ref(),
        }
    }

    fn entries_mut(&mut self, owner: Owner) -> Option<&mut Entries> {
        match owner {
            Owner::Profile => self.settings.as_mut().map(|settings| &mut settings.entries),
            Owner::Global => self.global.as_mut(),
        }
    }

    fn chosen_entry(&self) -> Option<&Entry> {
        let chosen = self.chosen?;
        self.entries(chosen.owner)?.get(chosen.index)
    }

    fn field(&self, field: Field) -> Option<&Entity<InputState>> {
        self.fields
            .iter()
            .find(|(other, _)| *other == field)
            .map(|(_, state)| state)
    }

    fn swatch(&self, swatch: Swatch) -> Option<&Entity<ColorPickerState>> {
        self.swatches
            .iter()
            .find(|(other, _)| *other == swatch)
            .map(|(_, picker)| picker)
    }

    fn sync_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(level) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.brightness)
        {
            let percent = percent_of(level);
            self.brightness
                .update(cx, |slider, cx| slider.set_value(percent, window, cx));
        }
        self.sync_entry_controls(window, cx);
    }

    fn sync_entry_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let entry = self.chosen_entry().cloned();
        for (swatch, picker) in &self.swatches {
            if let Some(color) = entry
                .as_ref()
                .and_then(|entry| swatch.color(&entry.effect, self.led))
            {
                picker.update(cx, |picker, cx| {
                    picker.set_value(hsla_of(color.rgb()), window, cx);
                });
            }
        }
        for (field, state) in &self.fields {
            if matches!(field, Field::HostId | Field::HostValue) {
                continue;
            }
            let text = entry
                .as_ref()
                .and_then(|entry| field.text(entry))
                .unwrap_or_default();
            state.update(cx, |state, cx| state.set_value(text, window, cx));
        }
        if let (Some(range), Some(slider)) = (
            entry
                .as_ref()
                .and_then(|entry| entry::lightbar_range(&entry.effect)),
            &self.range,
        ) {
            let value = (f32::from(range.first), f32::from(range.last));
            slider
                .state
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    fn field_changed(
        &mut self,
        field: Field,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.trim().is_empty() {
            return;
        }
        match field.parse(text) {
            Err(mistake) => {
                self.problem = Some(mistake.into());
                cx.notify();
            }
            Ok(_) if matches!(field, Field::HostId | Field::HostValue) => {
                self.problem = None;
                cx.notify();
            }
            Ok(number) => {
                self.problem = None;
                self.edit_chosen(|entry| field.apply(entry, number), window, cx);
            }
        }
    }

    fn edit_settings(
        &mut self,
        change: impl FnOnce(&mut lightbar::Settings),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(settings) = &mut self.settings else {
            return;
        };
        change(settings);
        self.pending.profile = true;
        self.schedule(window, cx);
        cx.notify();
    }

    fn edit_entries(
        &mut self,
        owner: Owner,
        change: impl FnOnce(&mut Entries),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entries) = self.entries_mut(owner) else {
            return;
        };
        change(entries);
        match owner {
            Owner::Profile => self.pending.profile = true,
            Owner::Global => self.pending.global = true,
        }
        self.schedule(window, cx);
        cx.notify();
    }

    fn edit_chosen(
        &mut self,
        change: impl FnOnce(&mut Entry),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(chosen) = self.chosen else {
            return;
        };
        self.edit_entries(
            chosen.owner,
            |entries| {
                if let Some(entry) = entries.get_mut(chosen.index) {
                    change(entry);
                }
            },
            window,
            cx,
        );
    }

    fn schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SEND_DELAY).await;
            this.update_in(cx, Self::flush).ok();
        }));
    }

    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.timer = None;
        let profile_changed = mem::take(&mut self.pending.profile);
        let global_changed = mem::take(&mut self.pending.global);
        if !profile_changed && !global_changed {
            return;
        }
        let (Some(profile), Some(capabilities)) = (self.profile, self.capabilities(cx)) else {
            return;
        };
        let mut problem = None;
        let mut sent = Vec::new();
        if profile_changed && let Some(settings) = self.settings.clone() {
            match settings.validate(&capabilities) {
                Ok(()) => sent.push(self.store.update(cx, |store, cx| {
                    store.apply::<SetLightbar>(
                        Change {
                            profile,
                            value: settings,
                        },
                        Scope::Profile(profile),
                        cx,
                    )
                })),
                Err(invalid) => problem = Some(invalid),
            }
        }
        if global_changed && let Some(global) = self.global.clone() {
            match global.validate(&capabilities) {
                Ok(()) => sent.push(self.store.update(cx, |store, cx| {
                    store.apply::<SetGlobalLightbar>(global, Scope::Keyboard, cx)
                })),
                Err(invalid) => problem = Some(invalid),
            }
        }
        self.problem = problem.map(|invalid| format!("Not sent: {invalid}").into());
        cx.notify();
        if sent.is_empty() {
            return;
        }
        let host_signals = capabilities.host_signals;
        cx.spawn_in(window, async move |this, cx| {
            let mut failed = false;
            for answer in sent {
                failed |= answer.await.is_err();
            }
            if failed {
                this.update_in(cx, Self::load).ok();
            } else if host_signals {
                this.update_in(cx, Self::refresh_host_ids).ok();
            }
        })
        .detach();
    }
}

impl Render for LightbarPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        panel::page()
            .children(self.preview(window, cx))
            .child(panel::heading(
                Page::Lightbar.title(),
                Some("The LED strip along the top edge of the keyboard.".into()),
                div(),
                cx,
            ))
            .when_some(self.problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(cx.theme().danger).child(problem))
            })
            .child(panel::columns(
                v_flex()
                    .gap_4()
                    .children(self.bar_card(cx))
                    .children(self.effects_card(cx)),
                v_flex()
                    .gap_4()
                    .children(self.effect_card(cx))
                    .children(self.condition_card(cx))
                    .children(self.host_card(cx)),
            ))
    }
}

fn first_entry(settings: Option<&lightbar::Settings>, global: Option<&Entries>) -> Option<Chosen> {
    if settings.is_some_and(|settings| !settings.entries.is_empty()) {
        Some(Chosen {
            owner: Owner::Profile,
            index: 0,
        })
    } else if global.is_some_and(|global| !global.is_empty()) {
        Some(Chosen {
            owner: Owner::Global,
            index: 0,
        })
    } else {
        None
    }
}

fn led_of(position: f32) -> u8 {
    let clamped = position.round().clamp(0.0, f32::from(u8::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=255"
    )]
    let led = clamped as u8;
    led
}
