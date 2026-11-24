mod brush;
mod color;
mod controls;
mod effect;
mod effect_editor;
mod key_colors;
mod layers;
mod lock;
mod painter;
mod palette;

use std::{mem, time::Duration};

use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window,
    component::{
        ActiveTheme as _,
        color_picker::{ColorPickerEvent, ColorPickerState},
        h_flex,
        slider::{SliderEvent, SliderState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    endpoints::{GetKeyColors, GetLighting, SetKeyColors, SetLighting},
    keys::{LAYERS, Layer},
    lighting::{self, Effect, KeyColors},
    profile::{Change, LayerChange, LayerRef, Slot},
};

use self::{brush::Brush, effect::Dial, lock::Lock};
pub(crate) use self::{
    color::{BLACK, WHITE, hsla_of, opaque_of},
    controls::{add_button, pill, segmented, select, stepper, sunken, tile, wide_slider},
    effect::{level_of, percent_of},
};
use super::Page;
use crate::{
    keyboard::Selection,
    panel,
    store::{Scope, Store, StoreEvent},
};

const SEND_DELAY: Duration = Duration::from_millis(120);
const COLOURS_WIDTH: f32 = 300.0;

type LayerMaps = [Option<KeyColors>; LAYERS];

pub(crate) struct LightingPage {
    store: Entity<Store>,
    profile: Option<Slot>,
    settings: Option<lighting::Settings>,
    maps: Option<LayerMaps>,
    layer: Layer,
    selection: Selection,
    picking: bool,
    effect: Option<usize>,
    problem: Option<SharedString>,
    brightness: Entity<SliderState>,
    brush: Brush,
    locks: Vec<(Lock, Entity<ColorPickerState>)>,
    dials: Vec<(Dial, Entity<SliderState>)>,
    first_color: Entity<ColorPickerState>,
    second_color: Entity<ColorPickerState>,
    generation: u64,
    pending: Pending,
    _subscriptions: Vec<Subscription>,
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
        let brush = Brush::new(window, cx);
        let first_color = cx.new(|cx| ColorPickerState::new(window, cx));
        let second_color = cx.new(|cx| ColorPickerState::new(window, cx));
        let locks: Vec<(Lock, Entity<ColorPickerState>)> = Lock::ALL
            .into_iter()
            .map(|lock| (lock, cx.new(|cx| ColorPickerState::new(window, cx))))
            .collect();
        let dials: Vec<(Dial, Entity<SliderState>)> = Dial::ALL
            .into_iter()
            .map(|dial| {
                let (min, max) = dial.bounds();
                (
                    dial,
                    cx.new(|_| SliderState::new().min(min).max(max).step(1.0)),
                )
            })
            .collect();
        let mut subscriptions = Self::subscriptions(
            &store,
            &brightness,
            [&first_color, &second_color],
            &locks,
            &dials,
            window,
            cx,
        );
        subscriptions.extend(brush.subscriptions(window, cx));
        let mut page = Self {
            store,
            profile: None,
            settings: None,
            maps: None,
            layer: Layer::Base,
            selection: Selection::default(),
            picking: false,
            effect: None,
            problem: None,
            brightness,
            brush,
            locks,
            dials,
            first_color,
            second_color,
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
        [first_color, second_color]: [&Entity<ColorPickerState>; 2],
        locks: &[(Lock, Entity<ColorPickerState>)],
        dials: &[(Dial, Entity<SliderState>)],
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
                        this.edit_settings(|settings| settings.brightness = level, window, cx);
                    }
                },
            ),
            cx.subscribe_in(
                first_color,
                window,
                |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = opaque_of(*color);
                        this.edit_effect(
                            |effect| effect::set_first_color(effect, color),
                            window,
                            cx,
                        );
                    }
                },
            ),
            cx.subscribe_in(
                second_color,
                window,
                |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = opaque_of(*color);
                        this.edit_effect(
                            |effect| effect::set_second_color(effect, color),
                            window,
                            cx,
                        );
                    }
                },
            ),
        ];
        for (lock, picker) in locks {
            let lock = *lock;
            subscriptions.push(cx.subscribe_in(
                picker,
                window,
                move |this, _, event: &ColorPickerEvent, window, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        let color = opaque_of(*color);
                        this.edit_settings(
                            |settings| lock.set(&mut settings.indicators, color),
                            window,
                            cx,
                        );
                    }
                },
            ));
        }
        for (dial, slider) in dials {
            let dial = *dial;
            subscriptions.push(cx.subscribe_in(
                slider,
                window,
                move |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
                        let position = value.end();
                        this.edit_effect(|effect| dial.apply(effect, position), window, cx);
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
        self.profile = self.store.read(cx).active();
        self.settings = None;
        self.maps = None;
        self.effect = None;
        self.problem = None;
        self.selection.clear();
        cx.notify();
        let Some(profile) = self.profile else {
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
                page.effect = settings
                    .as_ref()
                    .and_then(|settings| (!settings.effects.is_empty()).then_some(0));
                page.settings = settings;
                page.maps = maps.and_then(|maps| LayerMaps::try_from(maps).ok());
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
        self.maps = None;
        self.effect = None;
        self.problem = None;
        self.selection.clear();
        cx.notify();
    }

    fn capabilities(&self, cx: &Context<Self>) -> Option<Capabilities> {
        self.store
            .read(cx)
            .keyboard()
            .map(|keyboard| keyboard.capabilities.clone())
    }

    fn sync_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings {
            let brightness = percent_of(settings.brightness);
            self.brightness
                .update(cx, |slider, cx| slider.set_value(brightness, window, cx));
            for (lock, picker) in &self.locks {
                let color = lock.color(&settings.indicators);
                picker.update(cx, |picker, cx| {
                    if color.alpha == 0 {
                        picker.clear_value(window, cx);
                    } else {
                        picker.set_value(hsla_of(color.rgb()), window, cx);
                    }
                });
            }
        }
        self.sync_effect_controls(window, cx);
    }

    fn sync_effect_controls(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(effect) = self.selected_effect().copied() else {
            return;
        };
        for (dial, slider) in &self.dials {
            if let Some(position) = dial.position(&effect) {
                slider.update(cx, |slider, cx| slider.set_value(position, window, cx));
            }
        }
        let (first, second) = effect::colors(&effect);
        for (picker, color) in [(&self.first_color, first), (&self.second_color, second)] {
            if let Some(color) = color {
                picker.update(cx, |picker, cx| {
                    picker.set_value(hsla_of(color.rgb()), window, cx);
                });
            }
        }
    }

    fn selected_effect(&self) -> Option<&Effect> {
        let index = self.effect?;
        self.settings.as_ref()?.effects.get(index)
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
        change(settings);
        self.pending.lighting = true;
        self.schedule(window, cx);
        cx.notify();
    }

    fn edit_effect(
        &mut self,
        change: impl FnOnce(&mut Effect),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.effect else {
            return;
        };
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
            && let Some(settings) = self.settings.clone()
        {
            sent.push(self.store.update(cx, |store, cx| {
                store.apply::<SetLighting>(
                    Change {
                        profile,
                        value: settings,
                    },
                    Scope::Profile(profile),
                    cx,
                )
            }));
        }
        for layer in layers {
            let value = self.map(layer).cloned();
            if let Some(Err(invalid)) = value.as_ref().map(|map| map.validate(&capabilities)) {
                problem = Some(invalid);
                continue;
            }
            sent.push(self.store.update(cx, |store, cx| {
                store.apply::<SetKeyColors>(
                    LayerChange {
                        profile,
                        layer,
                        value,
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
        panel::page()
            .child(panel::stage_with_side(
                self.layers_column(cx),
                self.key_map(window, cx),
            ))
            .children(self.caption(cx))
            .child(panel::heading(
                Page::Lighting.title(),
                Some("Key colours for each layer, with effects drawn on top.".into()),
                self.heading_actions(cx),
                cx,
            ))
            .when_some(self.problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(cx.theme().danger).child(problem))
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
                            .gap_4()
                            .children(self.colors_card(cx))
                            .children(self.locks_card(cx)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_4()
                            .children(self.effects_card(window, cx)),
                    ),
            )
    }
}
