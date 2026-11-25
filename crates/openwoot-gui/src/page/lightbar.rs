mod active;
mod bar;
mod conditions;
mod entry;
mod host;
mod led_range;
mod live;
mod number;
mod parameters;
mod preset;
mod preview;
mod swatch;

use std::{
    mem,
    time::{Duration, Instant},
};

use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window,
    component::{
        ActiveTheme as _, h_flex,
        input::{InputEvent, InputState},
        slider::{SliderEvent, SliderState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    endpoints::{
        GetGlobalLightbar, GetHostSignals, GetLightbar, GetLighting, SetGlobalLightbar, SetLightbar,
    },
    lightbar::{self, Entries, Entry, HostSignal, HostSignalIds},
    lighting::Rgb,
    profile::{Change, Slot},
};

use self::{entry::Matrix, led_range::Grip, number::Field, swatch::Swatch};
use super::lighting::{level_of, percent_of};
use crate::{
    color_picker::{ColorPicker, ColorPickerEvent},
    panel,
    store::{Scope, Store, StoreEvent},
};

const SEND_DELAY: Duration = Duration::from_millis(150);
const PRESETS_WIDTH: f32 = 320.0;
const ACTIVE_WIDTH: f32 = 400.0;
const PERIOD_STEP_MS: f32 = 100.0;
const PERIOD_MAX_MS: f32 = 10_000.0;
const WHITE: Rgb = Rgb {
    red: u8::MAX,
    green: u8::MAX,
    blue: u8::MAX,
};

pub(crate) struct LightbarPage {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<lightbar::Settings>,
    global: Option<Entries>,
    sent: Sent,
    key_brightness: Option<u8>,
    chosen: Option<Chosen>,
    renaming: Option<Chosen>,
    owner: Owner,
    tab: Tab,
    define: bool,
    custom_source: bool,
    inherit_open: bool,
    led: u8,
    grip: Option<Grip>,
    host_ids: Option<HostSignalIds>,
    host_sent: Vec<HostSignal>,
    problem: Option<SharedString>,
    brightness: Entity<SliderState>,
    period: Entity<SliderState>,
    search: Entity<InputState>,
    rename: Entity<InputState>,
    pickers: Vec<(Swatch, Entity<ColorPicker>)>,
    fields: Vec<(Field, Entity<InputState>)>,
    generation: u64,
    pending: Pending,
    live: Option<lightbar::Colors>,
    rendered_at: Instant,
    watch: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Profile,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Presets,
    Creator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Chosen {
    owner: Owner,
    index: usize,
}

#[derive(Default)]
struct Sent {
    settings: Option<lightbar::Settings>,
    global: Option<Entries>,
}

#[derive(Default)]
struct Pending {
    profile: bool,
    global: bool,
    timer: Option<Task<()>>,
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
        let period = cx.new(|_| {
            SliderState::new()
                .min(PERIOD_STEP_MS)
                .max(PERIOD_MAX_MS)
                .step(PERIOD_STEP_MS)
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search effects"));
        let rename = cx.new(|cx| InputState::new(window, cx));
        let pickers = Self::color_pickers(window, cx);
        let fields: Vec<(Field, Entity<InputState>)> = Field::all()
            .into_iter()
            .map(|field| {
                let state =
                    cx.new(|cx| InputState::new(window, cx).placeholder(field.placeholder()));
                (field, state)
            })
            .collect();
        let mut subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::Reloaded | StoreEvent::ProfileChanged => {
                    this.load(window, cx);
                }
                StoreEvent::Disconnected => this.forget(cx),
                StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &brightness,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    let level = level_of(slid(event));
                    this.edit_settings(
                        |settings| {
                            if settings.brightness.is_some() {
                                settings.brightness = Some(level);
                            }
                        },
                        window,
                        cx,
                    );
                },
            ),
            cx.subscribe_in(
                &period,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    this.set_period(slid(event), window, cx);
                },
            ),
            cx.subscribe_in(&search, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        subscriptions.extend(Self::control_subscriptions(
            &pickers, &fields, &rename, window, cx,
        ));
        let mut page = Self {
            store,
            profile: None,
            settings: None,
            global: None,
            sent: Sent::default(),
            key_brightness: None,
            chosen: None,
            renaming: None,
            owner: Owner::Profile,
            tab: Tab::Presets,
            define: true,
            custom_source: false,
            inherit_open: true,
            led: 0,
            grip: None,
            host_ids: None,
            host_sent: Vec::new(),
            problem: None,
            brightness,
            period,
            search,
            rename,
            pickers,
            fields,
            generation: 0,
            pending: Pending::default(),
            live: None,
            rendered_at: Instant::now(),
            watch: None,
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn color_pickers(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<(Swatch, Entity<ColorPicker>)> {
        Swatch::ALL
            .into_iter()
            .map(|swatch| {
                let picker = cx.new(|cx| {
                    let picker = ColorPicker::new(WHITE, window, cx);
                    if swatch.allows_none() {
                        picker.allow_no_color()
                    } else {
                        picker
                    }
                });
                (swatch, picker)
            })
            .collect()
    }

    fn control_subscriptions(
        pickers: &[(Swatch, Entity<ColorPicker>)],
        fields: &[(Field, Entity<InputState>)],
        rename: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        let mut subscriptions = vec![cx.subscribe_in(
            rename,
            window,
            |this, state, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } | InputEvent::Blur => {
                    let text = state.read(cx).value();
                    this.finish_rename(&text, window, cx);
                }
                InputEvent::Change | InputEvent::Focus => {}
            },
        )];
        for (swatch, picker) in pickers {
            let swatch = *swatch;
            subscriptions.push(cx.subscribe_in(
                picker,
                window,
                move |this, _, event: &ColorPickerEvent, window, cx| {
                    let ColorPickerEvent::Change(color) = event;
                    let color = *color;
                    let (led, leds) = (this.led, this.leds(cx));
                    this.edit_chosen(
                        |entry| swatch.set(&mut entry.effect, led, leds, color),
                        window,
                        cx,
                    );
                },
            ));
        }
        for (field, state) in fields {
            let field = *field;
            subscriptions.push(cx.subscribe_in(
                state,
                window,
                move |this, state, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        let text = state.read(cx).value();
                        this.field_changed(field, &text, window, cx);
                    }
                    InputEvent::Change | InputEvent::Focus => {}
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
        let profile = self.store.read(cx).active().filter(|_| leds > 0);
        if profile != self.profile {
            self.chosen = None;
            self.renaming = None;
        }
        self.profile = profile;
        self.problem = None;
        cx.notify();
        let Some(profile) = profile else {
            self.forget(cx);
            return;
        };
        self.watch_bar(cx);
        let settings = self
            .store
            .update(cx, |store, cx| store.fetch::<GetLightbar, _>(profile, cx));
        let global = self
            .store
            .update(cx, |store, cx| store.fetch::<GetGlobalLightbar, _>((), cx));
        let lighting = self
            .store
            .update(cx, |store, cx| store.fetch::<GetLighting, _>(profile, cx));
        let ids = host_signals.then(|| {
            self.store
                .update(cx, |store, cx| store.fetch::<GetHostSignals, _>((), cx))
        });
        cx.spawn_in(window, async move |this, cx| {
            let settings = settings.await.ok();
            let global = global.await.ok();
            let key_brightness = lighting.await.ok().map(|lighting| lighting.brightness);
            let ids = match ids {
                Some(ids) => ids.await.ok(),
                None => None,
            };
            this.update_in(cx, |page, window, cx| {
                if page.generation != generation {
                    return;
                }
                page.settings = settings;
                page.global = global;
                page.sent = Sent {
                    settings: page.settings.clone(),
                    global: page.global.clone(),
                };
                page.key_brightness = key_brightness;
                page.host_ids = ids;
                page.keep_chosen();
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
        self.sent = Sent::default();
        self.chosen = None;
        self.renaming = None;
        self.host_ids = None;
        self.problem = None;
        self.stop_watching();
        cx.notify();
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

    fn keep_chosen(&mut self) {
        if let Some(chosen) = self.chosen
            && self
                .entries(chosen.owner)
                .is_none_or(|entries| chosen.index >= entries.len())
        {
            self.chosen = None;
        }
    }

    fn field(&self, field: Field) -> Option<&Entity<InputState>> {
        self.fields
            .iter()
            .find(|(other, _)| *other == field)
            .map(|(_, state)| state)
    }

    fn picker(&self, swatch: Swatch) -> Option<&Entity<ColorPicker>> {
        self.pickers
            .iter()
            .find(|(other, _)| *other == swatch)
            .map(|(_, picker)| picker)
    }

    fn sync_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let level = self
            .settings
            .as_ref()
            .and_then(|settings| settings.brightness)
            .or(self.key_brightness);
        if let Some(level) = level {
            let percent = percent_of(level);
            self.brightness
                .update(cx, |slider, cx| slider.set_value(percent, window, cx));
        }
        self.sync_entry_controls(window, cx);
    }

    fn sync_entry_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let entry = self.chosen_entry().cloned();
        for (swatch, picker) in &self.pickers {
            if let Some(entry) = &entry
                && let Some(shade) = swatch.color(&entry.effect, self.led)
            {
                picker.update(cx, |picker, cx| picker.set_value(shade.rgb(), window, cx));
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
        if let Some(period) = entry
            .as_ref()
            .and_then(|entry| entry::period_ms(&entry.effect))
        {
            let value = f32::from(period).clamp(PERIOD_STEP_MS, PERIOD_MAX_MS);
            self.period
                .update(cx, |slider, cx| slider.set_value(value, window, cx));
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
            self.sync_entry_controls(window, cx);
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
                self.sync_entry_controls(window, cx);
            }
        }
    }

    fn set_period(&mut self, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        let period = number::millis(value.clamp(PERIOD_STEP_MS, PERIOD_MAX_MS));
        self.edit_chosen(
            |entry| entry::set_period_ms(&mut entry.effect, period),
            window,
            cx,
        );
        if let Some(state) = self.field(Field::Period).cloned() {
            let text = number::seconds_text(period);
            state.update(cx, |state, cx| state.set_value(text, window, cx));
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
        let before = settings.clone();
        change(settings);
        if *settings == before {
            return;
        }
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
        let before = entries.clone();
        change(entries);
        if *entries == before {
            return;
        }
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
        if profile_changed
            && let (Some(before), Some(after)) = (self.sent.settings.clone(), self.settings.clone())
        {
            match after.validate(&capabilities) {
                Ok(()) => {
                    self.sent.settings = Some(after.clone());
                    sent.push(self.store.update(cx, |store, cx| {
                        store.change::<SetLightbar>(
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
                    }));
                }
                Err(invalid) => problem = Some(invalid),
            }
        }
        if global_changed
            && let (Some(before), Some(after)) = (self.sent.global.clone(), self.global.clone())
        {
            match after.validate(&capabilities) {
                Ok(()) => {
                    self.sent.global = Some(after.clone());
                    sent.push(self.store.update(cx, |store, cx| {
                        store.change::<SetGlobalLightbar>(&before, after, Scope::Keyboard, cx)
                    }));
                }
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
        self.rendered_at = Instant::now();
        let cards = match self.tab {
            Tab::Presets => h_flex()
                .w_full()
                .gap_4()
                .items_start()
                .child(
                    div()
                        .flex_none()
                        .w(px(PRESETS_WIDTH))
                        .children(self.presets_card(cx)),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(ACTIVE_WIDTH))
                        .children(self.active_card(cx)),
                )
                .child(div().flex_1().min_w_0().children(self.parameters_card(cx))),
            Tab::Creator => h_flex()
                .w_full()
                .gap_4()
                .items_start()
                .child(
                    div()
                        .flex_none()
                        .w(px(ACTIVE_WIDTH))
                        .children(self.active_card(cx)),
                )
                .child(div().flex_1().min_w_0().children(self.parameters_card(cx))),
        };
        let stage = panel::Stage::element(
            v_flex()
                .items_center()
                .gap_10()
                .children(self.preview(window, cx))
                .children(self.brightness_row(cx)),
        )
        .fit_content();
        let body = v_flex()
            .w_full()
            .gap_3()
            .when_some(self.problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(cx.theme().danger).child(problem))
            })
            .when(self.settings.is_some(), |this| this.child(cards))
            .children(self.host_card(cx));
        panel::page_with_stage("lightbar-body", stage, self.heading(cx), body)
    }
}

fn slid(event: &SliderEvent) -> f32 {
    match event {
        SliderEvent::Change(value) | SliderEvent::Release(value) => value.end(),
    }
}
