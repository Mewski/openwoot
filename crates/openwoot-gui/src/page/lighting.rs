mod controls;
mod effect;
mod effect_editor;
mod indicator;
mod key_colors;
mod layers;
mod painter;
mod palette;
mod parameters;

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
    advanced,
    configuration::{Capabilities, Validate as _},
    endpoints::{
        GetAdvancedKeys, GetKeyColors, GetKeySettings, GetKeymap, GetLighting, SetKeyColors,
        SetLighting,
    },
    keys::{self, Keymap, LAYERS, Layer, ScanMode},
    lighting::{self, Effect, KeyColors, Rgb},
    profile::{Change, LayerChange, LayerRef, Slot},
};

pub(crate) use self::{
    controls::{
        Choice, ICON_SIZE, LABEL_TEXT, PARAM_TEXT, add_button, choices, dial_row, kebab,
        param_label, pill, remove_button, stepper, sunken, tile, tiles_box, wide_slider,
    },
    effect::{level_of, percent_of},
};
use self::{effect::Dial, effect_editor::Focus, indicator::LockKeys};
use super::Page;
use crate::{
    color_picker::{ColorPicker, ColorPickerEvent},
    panel,
    store::{Scope, Store, StoreEvent},
};

const SEND_DELAY: Duration = Duration::from_millis(120);
const STALE_AFTER: Duration = Duration::from_secs(2);
const COLOURS_WIDTH: f32 = 301.0;
const CARD_PADDING: f32 = 16.0;
const WHITE: Rgb = Rgb {
    red: u8::MAX,
    green: u8::MAX,
    blue: u8::MAX,
};

type LayerMaps = [Option<KeyColors>; LAYERS];

pub(crate) struct LightingPage {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<lighting::Settings>,
    maps: Option<LayerMaps>,
    sent: Sent,
    locks: LockKeys,
    tachyon: bool,
    layer: Layer,
    focus: Focus,
    copied: Option<KeyColors>,
    problem: Option<SharedString>,
    brightness: Entity<SliderState>,
    brush: Entity<ColorPicker>,
    first_color: Entity<ColorPicker>,
    second_color: Entity<ColorPicker>,
    dials: Vec<DialControl>,
    generation: u64,
    pending: Pending,
    seen: Instant,
    _subscriptions: Vec<Subscription>,
}

struct DialControl {
    dial: Dial,
    slider: Entity<SliderState>,
    input: Entity<InputState>,
    _subscriptions: [Subscription; 2],
}

#[derive(Default)]
struct Sent {
    settings: Option<lighting::Settings>,
    maps: Option<LayerMaps>,
}

#[derive(Default)]
struct Pending {
    lighting: bool,
    layers: Vec<Layer>,
    timer: Option<Task<()>>,
}

impl LightingPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let brightness = cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0));
        let brush = cx.new(|cx| ColorPicker::new(WHITE, window, cx).allow_no_color());
        let first_color = cx.new(|cx| ColorPicker::new(WHITE, window, cx));
        let second_color = cx.new(|cx| ColorPicker::new(WHITE, window, cx));
        let subscriptions = vec![
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
                    let value = match event {
                        SliderEvent::Change(value) | SliderEvent::Release(value) => value.end(),
                    };
                    let level = level_of(value);
                    this.edit_settings(|settings| settings.brightness = level, window, cx);
                },
            ),
            cx.subscribe_in(&brush, window, |_, _, _: &ColorPickerEvent, _, cx| {
                cx.notify();
            }),
            cx.subscribe_in(
                &first_color,
                window,
                |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = color.opaque();
                        this.edit_focused(
                            |effect| effect::set_first_color(effect, color),
                            window,
                            cx,
                        );
                    }
                },
            ),
            cx.subscribe_in(
                &second_color,
                window,
                |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = color.opaque();
                        this.edit_focused(
                            |effect| effect::set_second_color(effect, color),
                            window,
                            cx,
                        );
                    }
                },
            ),
        ];
        let mut page = Self {
            store,
            profile: None,
            settings: None,
            maps: None,
            sent: Sent::default(),
            locks: LockKeys::default(),
            tachyon: false,
            layer: Layer::Base,
            focus: Focus::Base,
            copied: None,
            problem: None,
            brightness,
            brush,
            first_color,
            second_color,
            dials: Vec::new(),
            generation: 0,
            pending: Pending::default(),
            seen: Instant::now(),
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.flush(window, cx);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let profile = self.store.read(cx).active();
        if profile != self.profile {
            self.focus = Focus::Base;
            self.layer = Layer::Base;
        }
        self.profile = profile;
        self.problem = None;
        self.prepare_dials(window, cx);
        cx.notify();
        let Some(profile) = profile else {
            self.forget(cx);
            return;
        };
        let lighting = self
            .store
            .update(cx, |store, cx| store.fetch::<GetLighting, _>(profile, cx));
        let maps: Vec<_> = Layer::ALL
            .into_iter()
            .map(|layer| {
                self.store.update(cx, |store, cx| {
                    store.fetch::<GetKeyColors, _>(LayerRef { profile, layer }, cx)
                })
            })
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            let settings = lighting.await.ok();
            let mut loaded = Vec::with_capacity(LAYERS);
            for map in maps {
                loaded.push(map.await.ok());
            }
            let maps: Option<Vec<Option<KeyColors>>> = loaded.into_iter().collect();
            this.update_in(cx, |page, window, cx| {
                if page.generation != generation {
                    return;
                }
                page.settings = settings;
                page.maps = maps.and_then(|maps| LayerMaps::try_from(maps).ok());
                page.sent = Sent {
                    settings: page.settings.clone(),
                    maps: page.maps.clone(),
                };
                page.keep_focus();
                page.sync_controls(window, cx);
                page.refresh_key_state(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_key_state(&self, cx: &mut Context<Self>) {
        let Some(profile) = self.profile else {
            return;
        };
        let generation = self.generation;
        let device = self.store.read(cx).device();
        let keymaps: Vec<_> = Layer::ALL
            .into_iter()
            .map(|layer| device.configure::<GetKeymap, Keymap>(LayerRef { profile, layer }))
            .collect();
        let advanced = device.configure::<GetAdvancedKeys, advanced::Keys>(profile);
        let settings = device.configure::<GetKeySettings, keys::Settings>(profile);
        cx.spawn(async move |this, cx| {
            let mut loaded = Vec::with_capacity(LAYERS);
            for keymap in keymaps {
                let Ok(keymap) = keymap.await else {
                    return;
                };
                loaded.push(keymap);
            }
            let advanced = advanced.await.unwrap_or_default();
            let tachyon = settings
                .await
                .ok()
                .map(|settings| settings.scan_mode == ScanMode::Parallel);
            this.update(cx, |page, cx| {
                if page.generation != generation {
                    return;
                }
                let Some(keyboard) = page.store.read(cx).keyboard() else {
                    return;
                };
                let locks = LockKeys::find(&loaded, &advanced, |key| keyboard.has_key(key));
                let tachyon = tachyon.unwrap_or(page.tachyon);
                if page.locks != locks || page.tachyon != tachyon {
                    page.locks = locks;
                    page.tachyon = tachyon;
                    cx.notify();
                }
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
        self.maps = None;
        self.sent = Sent::default();
        self.locks = LockKeys::default();
        self.tachyon = false;
        self.focus = Focus::Base;
        self.problem = None;
        cx.notify();
    }

    fn capabilities(&self, cx: &Context<Self>) -> Option<Capabilities> {
        self.store
            .read(cx)
            .keyboard()
            .map(|keyboard| keyboard.capabilities.clone())
    }

    fn colors_card(&self, cx: &Context<Self>) -> Option<gpui_kit::Div> {
        self.maps.as_ref()?;
        Some(
            panel::section("Colours", None, cx)
                .p(px(CARD_PADDING))
                .child(self.brush.clone()),
        )
    }

    fn prepare_dials(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(capabilities) = self.capabilities(cx) else {
            return;
        };
        let current = self
            .dials
            .iter()
            .map(|control| {
                let slider = control.slider.read(cx);
                (control.dial, slider.min_value(), slider.max_value())
            })
            .collect::<Vec<_>>();
        let wanted = Dial::ALL
            .into_iter()
            .map(|dial| {
                let (min, max) = dial.bounds(&capabilities);
                (dial, min, max)
            })
            .collect::<Vec<_>>();
        if current == wanted {
            return;
        }
        self.dials = wanted
            .into_iter()
            .map(|(dial, min, max)| Self::dial_control(dial, min, max, window, cx))
            .collect();
    }

    fn dial_control(
        dial: Dial,
        min: f32,
        max: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DialControl {
        let slider = cx.new(|_| SliderState::new().min(min).max(max).step(1.0));
        let input = cx.new(|cx| InputState::new(window, cx));
        let moved = cx.subscribe_in(
            &slider,
            window,
            move |this, _, event: &SliderEvent, window, cx| {
                let raw = match event {
                    SliderEvent::Change(value) | SliderEvent::Release(value) => value.end(),
                };
                this.turn_dial(dial, raw, window, cx);
            },
        );
        let typed = cx.subscribe_in(
            &input,
            window,
            move |this, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    let text = input.read(cx).value();
                    this.type_dial(dial, &text, window, cx);
                }
            },
        );
        DialControl {
            dial,
            slider,
            input,
            _subscriptions: [moved, typed],
        }
    }

    fn dial(&self, dial: Dial) -> Option<&DialControl> {
        self.dials.iter().find(|control| control.dial == dial)
    }

    fn turn_dial(&mut self, dial: Dial, raw: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_focused(|effect| dial.apply(effect, raw), window, cx);
        if let Some(control) = self.dial(dial) {
            let text = dial.text(raw);
            control
                .input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
    }

    fn type_dial(&mut self, dial: Dial, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(capabilities) = self.capabilities(cx) else {
            return;
        };
        let typed = text
            .trim()
            .trim_end_matches(['%', 's'])
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite());
        if let Some(shown) = typed {
            let raw = dial.raw_of(shown, &capabilities);
            self.edit_focused(|effect| dial.apply(effect, raw), window, cx);
        }
        self.sync_effect_controls(window, cx);
    }

    fn sync_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings {
            let brightness = percent_of(settings.brightness);
            self.brightness
                .update(cx, |slider, cx| slider.set_value(brightness, window, cx));
        }
        self.sync_effect_controls(window, cx);
    }

    fn sync_effect_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(effect) = self.focused_effect().copied() else {
            return;
        };
        for control in &self.dials {
            if let Some(raw) = control.dial.raw(&effect) {
                control
                    .slider
                    .update(cx, |slider, cx| slider.set_value(raw, window, cx));
                let text = control.dial.text(raw);
                control
                    .input
                    .update(cx, |input, cx| input.set_value(text, window, cx));
            }
        }
        let (first, second) = effect::colors(&effect);
        for (picker, color) in [(&self.first_color, first), (&self.second_color, second)] {
            if let Some(color) = color {
                picker.update(cx, |picker, cx| picker.set_value(color.rgb(), window, cx));
            }
        }
    }

    fn edit_settings(
        &mut self,
        change: impl FnOnce(&mut lighting::Settings),
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
        self.pending.lighting = true;
        self.schedule(window, cx);
        cx.notify();
    }

    fn edit_effect(
        &mut self,
        index: usize,
        change: impl FnOnce(&mut Effect),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_settings(
            |settings| {
                if let Some(effect) = settings.effects.get_mut(index) {
                    change(effect);
                }
            },
            window,
            cx,
        );
    }

    fn edit_focused(
        &mut self,
        change: impl FnOnce(&mut Effect),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.focused_index() {
            self.edit_effect(index, change, window, cx);
        }
    }

    fn map(&self, layer: Layer) -> Option<&KeyColors> {
        self.maps
            .as_ref()?
            .get(usize::from(layer.number()))?
            .as_ref()
    }

    fn map_slot(&mut self, layer: Layer) -> Option<&mut Option<KeyColors>> {
        self.maps.as_mut()?.get_mut(usize::from(layer.number()))
    }

    fn mark_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending.layers.contains(&layer) {
            self.pending.layers.push(layer);
        }
        self.schedule(window, cx);
        cx.notify();
    }

    fn schedule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SEND_DELAY).await;
            this.update_in(cx, Self::flush).ok();
        }));
    }

    fn flush(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.timer = None;
        let lighting = mem::take(&mut self.pending.lighting);
        let layers = mem::take(&mut self.pending.layers);
        if !lighting && layers.is_empty() {
            return;
        }
        let (Some(profile), Some(capabilities)) = (self.profile, self.capabilities(cx)) else {
            return;
        };
        let mut problem = self
            .settings
            .as_ref()
            .and_then(|settings| settings.validate(&capabilities).err());
        let mut sent = Vec::new();
        if lighting
            && problem.is_none()
            && let (Some(before), Some(after)) = (self.sent.settings.clone(), self.settings.clone())
        {
            self.sent.settings = Some(after.clone());
            sent.push(self.store.update(cx, |store, cx| {
                store.change::<SetLighting>(
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
        for layer in layers {
            let after = self.map(layer).cloned();
            if let Some(Err(invalid)) = after.as_ref().map(|map| map.validate(&capabilities)) {
                problem = Some(invalid);
                continue;
            }
            let Some(slot) = self
                .sent
                .maps
                .as_mut()
                .and_then(|maps| maps.get_mut(usize::from(layer.number())))
            else {
                continue;
            };
            let before = mem::replace(slot, after.clone());
            sent.push(self.store.update(cx, |store, cx| {
                store.change::<SetKeyColors>(
                    &LayerChange {
                        profile,
                        layer,
                        value: before,
                    },
                    LayerChange {
                        profile,
                        layer,
                        value: after,
                    },
                    Scope::Profile(profile),
                    cx,
                )
            }));
        }
        self.problem = problem.map(|invalid| format!("Not sent: {invalid}").into());
        cx.notify();
        if sent.is_empty() {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let mut failed = false;
            for answer in sent {
                failed |= answer.await.is_err();
            }
            if failed {
                this.update_in(cx, Self::load).ok();
            }
        })
        .detach();
    }
}

impl Render for LightingPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.seen.elapsed() > STALE_AFTER && self.settings.is_some() {
            self.refresh_key_state(cx);
        }
        self.seen = Instant::now();
        let stage = self
            .key_map(cx)
            .map_or_else(|| panel::Stage::element(div()), panel::Stage::new)
            .side(self.layers_column(cx));
        let stage = match self.caption() {
            Some(caption) => stage.caption(caption, false),
            None => stage,
        };
        let body = v_flex()
            .w_full()
            .gap_4()
            .when_some(self.problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(cx.theme().danger).child(problem))
            })
            .when(self.tachyon, |this| {
                this.child(panel::warning(
                    "RGB effects are disabled while Tachyon Mode is enabled.",
                    cx,
                ))
            })
            .child(
                h_flex()
                    .w_full()
                    .gap_4()
                    .items_start()
                    .child(
                        v_flex()
                            .flex_none()
                            .w(px(COLOURS_WIDTH))
                            .children(self.colors_card(cx)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .children(self.effects_card(window, cx)),
                    ),
            );
        panel::page_with_stage(
            "lighting-body",
            stage,
            panel::heading(Page::Lighting.title()),
            body,
        )
    }
}
