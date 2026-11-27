mod creation;
mod editor;
mod kind;
mod overview;
mod preset;
mod rules;
mod tester;
mod timeline;

use std::time::{Duration, Instant};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, WindowExt as _,
        input::{InputEvent, InputState},
        notification::Notification,
        slider::{SliderEvent, SliderState},
        v_flex,
    },
};
use openwoot_protocol::{
    advanced::{self, Behaviour, MAX_ENTRIES},
    endpoints::{
        GetAdvancedKeys, GetKeyOverrides, GetKeySettings, GetKeymap, SetAdvancedKeys,
        SetKeySettings,
    },
    keys::{self, Action, Index, Keymap, LAYERS, Layer},
    profile::{Change, LayerRef},
};

pub(super) use self::kind::Kind;
use self::{creation::Sensing, preset::Preset, tester::Tester};
use super::{
    Page,
    performance::{Committed, DistanceField, Shown, Span},
    remap::{ActionPicker, PickerEvent, Target, layer_label, layer_pill, layers_column},
};
use crate::{
    keyboard::{Glyph, KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    theme, usage,
};

const HOLD_TIME_MIN_MS: f32 = 10.0;
const HOLD_TIME_MAX_MS: f32 = 1000.0;
const HOLD_TIME_STEP_MS: f32 = 10.0;
const DEFAULT_BOTTOM_OUT_SHARE: f32 = 0.9;
const BOTTOM_OUT_SNAP_MM: f32 = 0.025;
const STALE_AFTER: Duration = Duration::from_secs(2);
const ADVANCED_HELP: &str = "Advanced keys change the way your keyboard processes a keypress.";
const TAKEN: &str = "This key already has an advanced key on";

pub(crate) struct AdvancedPage {
    store: Entity<Store>,
    entries: Option<advanced::Keys>,
    keymaps: [Option<Keymap>; LAYERS],
    settings: Option<keys::Settings>,
    overrides: Option<keys::Overrides>,
    layer: Layer,
    mode: Mode,
    tab: Tab,
    slot: Option<ActionSlot>,
    picker: Entity<ActionPicker>,
    hold_time: Entity<SliderState>,
    hold_input: Entity<InputState>,
    bottom_out: Entity<DistanceField>,
    actuation: Entity<DistanceField>,
    press: Entity<DistanceField>,
    release: Entity<DistanceField>,
    revision: u64,
    seen: Instant,
    tester: Option<Tester>,
    tester_focus: FocusHandle,
    sensing: Option<Sensing>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Overview,
    Creating(Draft),
    Editing(Index, Layer),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Draft {
    kind: Kind,
    keys: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionSlot {
    Stroke(usize),
    Tap,
    Hold,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Bindings,
    Performance,
    Tester,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Distance {
    Actuation,
    PressSensitivity,
    ReleaseSensitivity,
}

impl Tab {
    const fn title(self) -> &'static str {
        match self {
            Self::Bindings => "Bindings",
            Self::Performance => "Performance",
            Self::Tester => "Tester",
            Self::Settings => "Advanced Settings",
        }
    }

    const fn icon(self) -> Catalog {
        match self {
            Self::Bindings => Catalog::Keyboard,
            Self::Performance => Catalog::Gauge,
            Self::Tester => Catalog::ScanLine,
            Self::Settings => Catalog::Settings,
        }
    }

    const fn of(kind: Kind) -> &'static [Self] {
        match kind {
            Kind::DynamicKeystroke | Kind::ModTap => {
                &[Self::Bindings, Self::Tester, Self::Settings]
            }
            Kind::Toggle => &[Self::Bindings, Self::Tester],
            Kind::DepthPriority | Kind::Socd => &[Self::Performance, Self::Tester],
        }
    }
}

impl AdvancedPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| ActionPicker::new(&store, window, cx).flush());
        let hold_time = cx.new(|_| {
            SliderState::new()
                .min(HOLD_TIME_MIN_MS)
                .max(HOLD_TIME_MAX_MS)
                .step(HOLD_TIME_STEP_MS)
                .default_value(f32::from(advanced::ModTap::DEFAULT_HOLD_TIME_MS))
        });
        let hold_input = cx.new(|cx| InputState::new(window, cx));
        let bottom_out = cx.new(|cx| DistanceField::labelled(window, cx));
        let actuation = cx.new(|cx| DistanceField::labelled(window, cx));
        let press = cx.new(|cx| DistanceField::horizontal(window, cx));
        let release = cx.new(|cx| DistanceField::horizontal(window, cx));
        let subscriptions = vec![
            Self::store_subscription(&store, window, cx),
            Self::picker_subscription(&picker, window, cx),
            Self::hold_time_subscription(&hold_time, window, cx),
            Self::hold_input_subscription(&hold_input, window, cx),
            cx.subscribe_in(
                &bottom_out,
                window,
                |this, _, Committed(millimetres): &Committed, window, cx| {
                    this.commit_bottom_out(*millimetres, window, cx);
                },
            ),
            Self::distance_subscription(&actuation, Distance::Actuation, window, cx),
            Self::distance_subscription(&press, Distance::PressSensitivity, window, cx),
            Self::distance_subscription(&release, Distance::ReleaseSensitivity, window, cx),
        ];
        let mut page = Self {
            store,
            entries: None,
            keymaps: [const { None }; LAYERS],
            settings: None,
            overrides: None,
            layer: Layer::Base,
            mode: Mode::Overview,
            tab: Tab::Bindings,
            slot: None,
            picker,
            hold_time,
            hold_input,
            bottom_out,
            actuation,
            press,
            release,
            revision: 0,
            seen: Instant::now(),
            tester: None,
            tester_focus: cx.focus_handle(),
            sensing: None,
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn store_subscription(
        store: &Entity<Store>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(store, window, |this, _, event, window, cx| match event {
            StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                this.load(window, cx);
            }
            StoreEvent::Disconnected => {
                this.entries = None;
                this.keymaps = [const { None }; LAYERS];
                this.settings = None;
                this.overrides = None;
                this.overview(cx);
            }
            StoreEvent::Failed(_) => {}
        })
    }

    fn picker_subscription(
        picker: &Entity<ActionPicker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            picker,
            window,
            |this, _, event: &PickerEvent, window, cx| match event {
                PickerEvent::Picked(action) => {
                    if let Some(slot) = this.slot {
                        this.set_slot_action(slot, *action, window, cx);
                    }
                }
                PickerEvent::Cancelled => this.choose_slot(None, window, cx),
            },
        )
    }

    fn hold_time_subscription(
        slider: &Entity<SliderState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            slider,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(value) => {
                    let milliseconds = milliseconds_of(value.end());
                    this.hold_input.update(cx, |input, cx| {
                        input.set_value(milliseconds.to_string(), window, cx);
                    });
                }
                SliderEvent::Release(value) => {
                    this.set_hold_time(milliseconds_of(value.end()), window, cx);
                }
            },
        )
    }

    fn hold_input_subscription(
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            input,
            window,
            |this, input, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    return;
                }
                match input.read(cx).value().trim().parse::<f32>() {
                    Ok(typed) if typed.is_finite() => {
                        this.set_hold_time(milliseconds_of(typed), window, cx);
                    }
                    _ => this.sync_sliders(window, cx),
                }
            },
        )
    }

    fn distance_subscription(
        field: &Entity<DistanceField>,
        distance: Distance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            field,
            window,
            move |this, _, Committed(millimetres): &Committed, window, cx| {
                this.set_distance(distance, *millimetres, window, cx);
            },
        )
    }

    fn set_hold_time(&mut self, milliseconds: u16, window: &mut Window, cx: &mut Context<Self>) {
        self.change_edited(
            |entry| {
                if let Behaviour::ModTap(mod_tap) = &mut entry.behaviour {
                    mod_tap.hold_time_ms = milliseconds;
                }
            },
            window,
            cx,
        );
        self.sync_sliders(window, cx);
    }

    fn commit_bottom_out(&mut self, millimetres: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let key = self
            .edited()
            .map_or(0, |entry| usize::from(entry.key.get()));
        let default = keyboard.travel_mm(key) * DEFAULT_BOTTOM_OUT_SHARE;
        let custom = ((millimetres - default).abs() > BOTTOM_OUT_SNAP_MM).then_some(millimetres);
        self.set_bottom_out(custom, window, cx);
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let profile = keyboard.active;
        let entries = self.store.update(cx, |store, cx| {
            store.fetch::<GetAdvancedKeys, _>(profile, cx)
        });
        let keymaps = Layer::ALL.map(|layer| {
            self.store.update(cx, |store, cx| {
                store.fetch::<GetKeymap, _>(LayerRef { profile, layer }, cx)
            })
        });
        let settings = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeySettings, _>(profile, cx)
        });
        let overrides = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeyOverrides, _>(profile, cx)
        });
        let revision = self.revision;
        cx.spawn_in(window, async move |this, cx| {
            let entries = entries.await.ok();
            let mut loaded = [const { None }; LAYERS];
            for (slot, keymap) in loaded.iter_mut().zip(keymaps) {
                *slot = keymap.await.ok();
            }
            let settings = settings.await.ok();
            let overrides = overrides.await.ok();
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    page.load(window, cx);
                    return;
                }
                page.entries = entries;
                page.keymaps = loaded;
                page.settings = settings;
                page.overrides = overrides;
                if let Mode::Editing(key, layer) = page.mode
                    && page.entry(key, layer).is_none()
                {
                    page.overview(cx);
                }
                page.sync_sliders(window, cx);
                page.sync_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reload_entries(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let entries = self.store.update(cx, |store, cx| {
            store.fetch::<GetAdvancedKeys, _>(profile, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let entries = entries.await.ok();
            this.update_in(cx, |page, window, cx| {
                page.entries = entries;
                if let Mode::Editing(key, layer) = page.mode
                    && page.entry(key, layer).is_none()
                {
                    page.overview(cx);
                }
                page.sync_sliders(window, cx);
                page.sync_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_keymaps(&self, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let device = self.store.read(cx).device();
        let calls = Layer::ALL
            .map(|layer| device.configure::<GetKeymap, Keymap>(LayerRef { profile, layer }));
        cx.spawn(async move |this, cx| {
            let mut loaded = [const { None }; LAYERS];
            for (slot, call) in loaded.iter_mut().zip(calls) {
                *slot = call.await.ok();
            }
            this.update(cx, |page, cx| {
                if loaded.iter().all(Option::is_some) && page.keymaps != loaded {
                    page.keymaps = loaded;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn entries(&self) -> &[advanced::Key] {
        self.entries.as_deref().unwrap_or_default()
    }

    fn entry(&self, key: Index, layer: Layer) -> Option<&advanced::Key> {
        self.entries()
            .iter()
            .find(|entry| entry.key == key && entry.layer == layer)
    }

    fn edited(&self) -> Option<advanced::Key> {
        match self.mode {
            Mode::Editing(key, layer) => self.entry(key, layer).copied(),
            Mode::Overview | Mode::Creating(_) => None,
        }
    }

    fn holder(&self, key: usize, layer: Layer) -> Option<&advanced::Key> {
        let index = u8::try_from(key).ok().map(Index::new)?;
        rules::holder(self.entries(), index, layer)
            .and_then(|position| self.entries().get(position))
    }

    fn action_of(&self, layer: Layer, key: usize) -> Action {
        let on = |layer: Layer| {
            self.keymaps
                .get(usize::from(layer.number()))
                .and_then(Option::as_ref)
                .and_then(|keymap| keymap.get(key).copied())
        };
        match on(layer) {
            Some(Action::None) | None if layer != Layer::Base => on(Layer::Base),
            action => action,
        }
        .unwrap_or_default()
    }

    fn key_name(&self, key: usize) -> String {
        let legend = usage::legend(self.action_of(Layer::Base, key));
        if legend.is_empty() {
            format!("key {}", key + 1)
        } else {
            legend.into_owned()
        }
    }

    fn legend(&self, action: Action, cx: &App) -> SharedString {
        self.store.read(cx).keyboard().map_or_else(
            || SharedString::from(usage::legend(action)),
            |keyboard| keyboard.legend(action),
        )
    }

    fn index_name(&self, index: Index) -> String {
        self.key_name(usize::from(index.get()))
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let layer = self.layer;
        let target = self
            .edited()
            .zip(self.slot)
            .and_then(|(entry, slot)| slot_action(&entry, slot))
            .map(|current| Target { current });
        self.picker.update(cx, |picker, cx| {
            picker.set_layer(layer, cx);
            picker.set_target(target, cx);
        });
    }

    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let key = self
            .edited()
            .map_or(0, |entry| usize::from(entry.key.get()));
        let travel = keyboard.travel_mm(key);
        let shown = |millimetres: f32, span: Span, disabled: bool| Shown {
            millimetres,
            span,
            default: None,
            mixed: false,
            disabled,
        };
        if let Some(settings) = self.settings {
            let resting = !settings.rapid_trigger_enabled;
            let press = settings.rapid_trigger.press;
            let fields = [
                (
                    &self.actuation,
                    shown(
                        keyboard.distance_mm(key, settings.actuation),
                        Span::actuation(travel),
                        false,
                    ),
                ),
                (
                    &self.press,
                    shown(
                        keyboard.distance_mm(key, press),
                        Span::sensitivity(travel),
                        resting,
                    ),
                ),
                (
                    &self.release,
                    shown(
                        keyboard.distance_mm(key, settings.rapid_trigger.release.unwrap_or(press)),
                        Span::sensitivity(travel),
                        resting,
                    ),
                ),
            ];
            for (field, shown) in fields {
                field.update(cx, |field, cx| field.show(shown, window, cx));
            }
        }
        let Some(entry) = self.edited() else {
            return;
        };
        match entry.behaviour {
            Behaviour::ModTap(mod_tap) => {
                let milliseconds = mod_tap.hold_time_ms;
                self.hold_time.update(cx, |slider, cx| {
                    slider.set_value(f32::from(milliseconds), window, cx);
                });
                self.hold_input.update(cx, |input, cx| {
                    input.set_value(milliseconds.to_string(), window, cx);
                });
            }
            Behaviour::DynamicKeystroke(keystroke) => {
                let default = travel * DEFAULT_BOTTOM_OUT_SHARE;
                let bottom = keystroke
                    .bottom_out
                    .map_or(default, |distance| keyboard.distance_mm(key, distance));
                self.bottom_out.update(cx, |field, cx| {
                    field.show(
                        Shown {
                            default: Some(default),
                            ..shown(bottom, Span::actuation(travel), false)
                        },
                        window,
                        cx,
                    );
                });
            }
            Behaviour::Toggle(_) | Behaviour::DepthPriority(_) | Behaviour::Socd(_) => {}
        }
    }

    fn choose_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        if self.layer != layer {
            self.layer = layer;
            if let Mode::Creating(draft) = &mut self.mode {
                draft.keys.clear();
            } else {
                self.overview(cx);
            }
        }
        self.sync_picker(cx);
        cx.notify();
    }

    fn overview(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Overview;
        self.slot = None;
        self.tester = None;
        self.sensing = None;
        self.sync_picker(cx);
        cx.notify();
    }

    fn edit(&mut self, key: Index, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let reopened = self.mode == Mode::Editing(key, layer);
        if !reopened {
            self.tester = None;
            self.slot = None;
        }
        self.sensing = None;
        self.layer = layer;
        self.mode = Mode::Editing(key, layer);
        if let Some(entry) = self.entry(key, layer) {
            let tabs = Tab::of(Kind::of(&entry.behaviour));
            if !reopened || !tabs.contains(&self.tab) {
                self.tab = tabs.first().copied().unwrap_or(Tab::Tester);
            }
        }
        self.sync_sliders(window, cx);
        self.sync_picker(cx);
        if self.tab == Tab::Tester {
            self.start_test(window, cx);
        }
        cx.notify();
    }

    fn choose_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::Tester {
            self.start_test(window, cx);
        } else {
            self.stop_test(cx);
        }
        cx.notify();
    }

    fn choose_slot(
        &mut self,
        slot: Option<ActionSlot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.slot = slot;
        if slot.is_some() && self.tab != Tab::Bindings {
            self.choose_tab(Tab::Bindings, window, cx);
        }
        self.sync_picker(cx);
        if slot.is_some() {
            self.picker
                .update(cx, |picker, cx| picker.listen(window, cx));
        } else {
            self.picker.update(cx, ActionPicker::stop_listening);
        }
        cx.notify();
    }

    fn click_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        if let Mode::Creating(_) = self.mode {
            self.draft_key(key, cx);
            return;
        }
        if let Some(entry) = self.holder(key, layer).copied() {
            self.edit(entry.key, entry.layer, window, cx);
        }
    }

    fn write(
        &mut self,
        entries: &[advanced::Key],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return false;
        };
        let profile = keyboard.active;
        let checked = rules::check(
            entries,
            &keyboard.capabilities,
            |index| self.index_name(index),
            layer_phrase,
        );
        if let Err(problem) = checked {
            window.push_notification(Notification::error(sentence(&problem.to_string())), cx);
            return false;
        }
        let Ok(value) = advanced::Keys::from_slice(entries) else {
            return false;
        };
        let before = self.entries.clone().unwrap_or_default();
        self.entries = Some(value.clone());
        self.revision += 1;
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetAdvancedKeys>(
                &Change {
                    profile,
                    value: before,
                },
                Change { profile, value },
                Scope::Profile(profile),
                cx,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            if applied.await.is_err() {
                this.update_in(cx, Self::reload_entries).ok();
            }
        })
        .detach();
        self.sync_picker(cx);
        cx.notify();
        true
    }

    fn change_edited(
        &mut self,
        change: impl FnOnce(&mut advanced::Key),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Editing(key, layer) = self.mode else {
            return;
        };
        let mut entries = self.entries().to_vec();
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.key == key && entry.layer == layer)
        else {
            return;
        };
        let before = *entry;
        change(entry);
        if *entry != before && !self.write(&entries, window, cx) {
            self.sync_sliders(window, cx);
        }
    }

    fn set_slot_action(
        &mut self,
        slot: ActionSlot,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_edited(
            |entry| match (&mut entry.behaviour, slot) {
                (Behaviour::DynamicKeystroke(keystroke), ActionSlot::Stroke(stroke)) => {
                    if let Some(stroke) = keystroke.strokes.get_mut(stroke) {
                        stroke.action = action;
                    }
                }
                (Behaviour::ModTap(mod_tap), ActionSlot::Tap) => mod_tap.tap = action,
                (Behaviour::ModTap(mod_tap), ActionSlot::Hold) => mod_tap.hold = action,
                (Behaviour::Toggle(toggled), ActionSlot::Toggle) => *toggled = action,
                _ => {}
            },
            window,
            cx,
        );
        self.slot = None;
        self.picker.update(cx, ActionPicker::stop_listening);
        self.sync_picker(cx);
        cx.notify();
    }

    fn set_bottom_out(
        &mut self,
        millimetres: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        self.change_edited(
            |entry| {
                let key = usize::from(entry.key.get());
                if let Behaviour::DynamicKeystroke(keystroke) = &mut entry.behaviour {
                    keystroke.bottom_out =
                        millimetres.map(|millimetres| keyboard.distance_from_mm(key, millimetres));
                }
            },
            window,
            cx,
        );
        self.sync_sliders(window, cx);
    }

    fn change_settings(
        &mut self,
        change: impl FnOnce(&mut keys::Settings, &Keyboard, usize),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(keyboard), Some(before)) =
            (self.store.read(cx).keyboard().cloned(), self.settings)
        else {
            return;
        };
        let key = self
            .edited()
            .map_or(0, |entry| usize::from(entry.key.get()));
        let mut after = before;
        change(&mut after, &keyboard, key);
        if after == before {
            return;
        }
        let profile = keyboard.active;
        self.settings = Some(after);
        let applied = self.store.update(cx, |store, cx| {
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
        });
        cx.spawn_in(window, async move |this, cx| {
            if applied.await.is_err() {
                this.update_in(cx, |page, window, cx| {
                    page.settings = Some(before);
                    page.sync_sliders(window, cx);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        self.sync_sliders(window, cx);
        cx.notify();
    }

    fn set_distance(
        &mut self,
        distance: Distance,
        millimetres: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_settings(
            |settings, keyboard, key| {
                let value = keyboard.distance_from_mm(key, millimetres);
                match distance {
                    Distance::Actuation => settings.actuation = value,
                    Distance::PressSensitivity => settings.rapid_trigger.press = value,
                    Distance::ReleaseSensitivity => settings.rapid_trigger.release = Some(value),
                }
            },
            window,
            cx,
        );
    }

    fn delete(&mut self, key: Index, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Vec<advanced::Key> = self
            .entries()
            .iter()
            .filter(|entry| !(entry.key == key && entry.layer == layer))
            .copied()
            .collect();
        if self.mode == Mode::Editing(key, layer) {
            self.overview(cx);
        }
        self.write(&entries, window, cx);
    }

    fn confirm_delete(entry: advanced::Key, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let title = format!(
            "Remove this {} binding?",
            Kind::of(&entry.behaviour).title()
        );
        panel::confirm(
            panel::Confirmation::new(
                title,
                "Are you sure you want to remove this advanced key?",
                "Remove",
            )
            .danger()
            .alert(),
            move |window, cx| {
                page.update(cx, |page, cx| {
                    page.delete(entry.key, entry.layer, window, cx);
                })
                .ok();
            },
            window,
            cx,
        );
    }

    fn preset_plan(&self, preset: &Preset) -> Option<(Vec<advanced::Key>, Vec<advanced::Key>)> {
        let base = self.keymaps.first().and_then(Option::as_ref)?;
        let added = preset.entries(base).ok()?;
        let replaced: Vec<advanced::Key> = self
            .entries()
            .iter()
            .filter(|entry| {
                rules::bindings(entry).any(|binding| {
                    added
                        .iter()
                        .any(|new| rules::bindings(new).any(|taken| taken == binding))
                })
            })
            .copied()
            .collect();
        let total = self.entries().len() - replaced.len() + added.len();
        (total <= MAX_ENTRIES).then_some((added, replaced))
    }

    fn apply_preset(&mut self, preset: &Preset, window: &mut Window, cx: &mut Context<Self>) {
        let Some((added, replaced)) = self.preset_plan(preset) else {
            return;
        };
        let mut entries: Vec<advanced::Key> = self
            .entries()
            .iter()
            .filter(|entry| !replaced.contains(entry))
            .copied()
            .collect();
        entries.extend(added.iter().copied());
        if self.write(&entries, window, cx) {
            self.overview(cx);
        }
    }

    fn confirm_preset(
        &mut self,
        preset: &'static Preset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((_, replaced)) = self.preset_plan(preset) else {
            return;
        };
        if replaced.is_empty() {
            self.apply_preset(preset, window, cx);
            return;
        }
        let page = cx.entity().downgrade();
        let current: Vec<String> = replaced
            .iter()
            .map(|entry| {
                format!(
                    "{} on {}",
                    Kind::of(&entry.behaviour).short_title(),
                    self.keys_name(entry)
                )
            })
            .collect();
        let title = match replaced.len() {
            1 => "This preset will replace an existing Advanced Key".to_owned(),
            count => format!("This preset will replace {count} existing Advanced Keys"),
        };
        panel::confirm(
            panel::Confirmation::new(
                title,
                format!("Replaces {}.", listing(&current)),
                "Apply preset",
            ),
            move |window, cx| {
                page.update(cx, |page, cx| page.apply_preset(preset, window, cx))
                    .ok();
            },
            window,
            cx,
        );
    }

    fn keys_name(&self, entry: &advanced::Key) -> String {
        let names: Vec<String> = rules::bindings(entry)
            .map(|(key, _)| self.index_name(key))
            .collect();
        names.join(" + ")
    }

    fn face(&self, keyboard: &Keyboard, key: usize, cx: &App) -> KeyFace {
        let layer = self.layer;
        let theme = cx.theme();
        let caps = theme::keycaps();
        let on = |layer: Layer| {
            self.keymaps
                .get(usize::from(layer.number()))
                .and_then(Option::as_ref)
                .and_then(|keymap| keymap.get(key).copied())
        };
        let base = on(Layer::Base)
            .or_else(|| keyboard.legends.get(key).copied())
            .unwrap_or_default();
        let own = if layer == Layer::Base {
            Some(base)
        } else {
            on(layer)
        };
        let shown = match own {
            Some(Action::None) | None => base,
            Some(action) => action,
        };
        let legend = keyboard.legend(shown);
        let mut face = KeyFace {
            glyph: super::remap::glyph(shown, legend.clone(), caps.legend, theme.primary),
            legend,
            dimmed: matches!(own, Some(Action::None) | None),
            ..KeyFace::default()
        };
        let holder = self.holder(key, layer).copied();
        if let Some(entry) = holder {
            face.glyph = Some(Kind::of(&entry.behaviour).glyph(caps.legend));
            face.dimmed = false;
        }
        if let Mode::Creating(draft) = &self.mode {
            let chosen = draft.keys.contains(&key);
            if let Some(entry) = holder {
                face.tooltip = Some(format!("{TAKEN} '{}'", layer_label(entry.layer)).into());
            } else if chosen {
                face.hover_glyph = Some(mark(Catalog::CircleX, theme.danger));
            } else {
                face.hover_glyph = Some(mark(Catalog::CirclePlus, theme.success));
            }
        }
        face.travel = self.tested_travel(key);
        face
    }

    fn view_selection(&self) -> Selection {
        let mut selection = Selection::default();
        match &self.mode {
            Mode::Creating(draft) => selection.set(draft.keys.iter().copied()),
            Mode::Editing(..) => {
                if let Some(entry) = self.edited() {
                    selection.set(rules::bindings(&entry).map(|(key, _)| usize::from(key.get())));
                }
            }
            Mode::Overview => {}
        }
        selection
    }

    fn shown(&self, layer: Layer) -> bool {
        let keymap = |layer: Layer| {
            self.keymaps
                .get(usize::from(layer.number()))
                .and_then(Option::as_ref)
        };
        layer == Layer::Base
            || layer == self.layer
            || self.entries().iter().any(|entry| entry.layer == layer)
            || keymap(layer)
                .is_some_and(|keymap| keymap.iter().any(|action| *action != Action::None))
            || self
                .keymaps
                .iter()
                .flatten()
                .any(|keymap| keymap.contains(&Action::Layer(layer)))
    }

    fn layers(&self, cx: &mut Context<Self>) -> Div {
        let pills: Vec<AnyElement> = Layer::ALL
            .into_iter()
            .filter(|layer| self.shown(*layer))
            .map(|layer| {
                layer_pill(
                    format!("advanced-layer-{}", layer.number()),
                    layer,
                    self.layer == layer,
                    None,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.choose_layer(layer, cx)))
                .into_any_element()
            })
            .collect();
        layers_column(pills, None, cx)
    }
}

impl Render for AdvancedPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editing = matches!(self.mode, Mode::Editing(..));
        if panel::focused(cx) != editing {
            panel::set_focus(editing, cx);
            window.refresh();
        }
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page();
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_keymaps(cx);
        }
        self.seen = Instant::now();
        let weak = cx.entity().downgrade();
        let view = KeyboardView::new("advanced-keyboard", &keyboard, |key| {
            self.face(&keyboard, key, cx)
        })
        .selection(&self.view_selection())
        .on_key_click(move |key, _, window, cx| {
            weak.update(cx, |page, cx| page.click_key(key, window, cx))
                .ok();
        });
        let stage = panel::Stage::new(view).side(self.layers(cx)).focus(editing);
        match self.mode.clone() {
            Mode::Creating(draft) => {
                panel::page_with_stage_only("advanced-body", stage, self.creation_card(&draft, cx))
            }
            Mode::Editing(..) => panel::page().child(panel::scroll_area(
                "advanced-body",
                v_flex().w_full().child(stage).children(
                    self.edited().map(|entry| {
                        panel::raised(self.editor_card(&entry, &keyboard, window, cx))
                    }),
                ),
            )),
            Mode::Overview => panel::page_with_stage(
                "advanced-body",
                stage,
                panel::heading(Page::Advanced.title()).help(ADVANCED_HELP),
                panel::columns(self.add_card(cx), self.active_card(cx)),
            ),
        }
    }
}

fn mark(icon: Catalog, color: gpui_kit::Hsla) -> Glyph {
    Glyph::new(move |height| {
        Icon::new(icon)
            .size(height * 0.5)
            .text_color(color)
            .into_any_element()
    })
}

fn slot_action(entry: &advanced::Key, slot: ActionSlot) -> Option<Action> {
    match (entry.behaviour, slot) {
        (Behaviour::DynamicKeystroke(keystroke), ActionSlot::Stroke(stroke)) => {
            keystroke.strokes.get(stroke).map(|stroke| stroke.action)
        }
        (Behaviour::ModTap(mod_tap), ActionSlot::Tap) => Some(mod_tap.tap),
        (Behaviour::ModTap(mod_tap), ActionSlot::Hold) => Some(mod_tap.hold),
        (Behaviour::Toggle(action), ActionSlot::Toggle) => Some(action),
        _ => None,
    }
}

fn milliseconds_of(value: f32) -> u16 {
    let clamped = value.round().clamp(HOLD_TIME_MIN_MS, HOLD_TIME_MAX_MS);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 10..=1000"
    )]
    let milliseconds = clamped as u16;
    milliseconds
}

const fn layer_phrase(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "the main layer",
        Layer::One => "Fn layer 1",
        Layer::Two => "Fn layer 2",
        Layer::Three => "Fn layer 3",
    }
}

fn sentence(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

fn listing(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_durations_stay_within_the_slider_range() {
        assert_eq!(milliseconds_of(204.6), 205);
        assert_eq!(milliseconds_of(0.0), 10);
        assert_eq!(milliseconds_of(5000.0), 1000);
    }

    #[test]
    fn every_kind_has_its_wootility_tabs() {
        assert_eq!(
            Tab::of(Kind::DynamicKeystroke),
            &[Tab::Bindings, Tab::Tester, Tab::Settings]
        );
        assert_eq!(Tab::of(Kind::Toggle), &[Tab::Bindings, Tab::Tester]);
        assert_eq!(Tab::of(Kind::Socd), &[Tab::Performance, Tab::Tester]);
    }

    #[test]
    fn listings_join_the_last_item_with_and() {
        let items = |names: &[&str]| {
            names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(listing(&items(&["Mod Tap on Caps"])), "Mod Tap on Caps");
        assert_eq!(listing(&items(&["A", "B", "C"])), "A, B and C");
    }
}
