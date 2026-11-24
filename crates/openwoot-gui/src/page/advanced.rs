mod editor;
mod kind;
mod preset;
mod rules;
mod tester;

use std::time::{Duration, Instant};

use gpui_kit::{
    AppContext as _, Context, Div, Entity, FocusHandle, FontWeight, Hsla, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        notification::Notification,
        slider::{SliderEvent, SliderState},
        tab::{Tab, TabBar},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, MAX_ENTRIES},
    endpoints::{GetAdvancedKeys, GetKeymap, SetAdvancedKeys},
    keys::{Action, Index, Keymap, LAYERS, Layer},
    profile::{Change, LayerRef},
};

pub(super) use self::kind::Kind;
use self::preset::{PRESETS, Preset};
use super::remap::{ActionPicker, PickerEvent, Target};
use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const KEYBOARD_MARGIN: f32 = 340.0;
const HOLD_TIME_MIN_MS: f32 = 10.0;
const HOLD_TIME_MAX_MS: f32 = 1000.0;
const HOLD_TIME_STEP_MS: f32 = 10.0;
const BOTTOM_OUT_MIN_MM: f32 = 0.1;
const BOTTOM_OUT_MAX_MM: f32 = 4.0;
const BOTTOM_OUT_STEP_MM: f32 = 0.05;
const DEFAULT_BOTTOM_OUT_SHARE: f32 = 0.9;
const STALE_AFTER: Duration = Duration::from_secs(2);

pub(crate) struct AdvancedPage {
    store: Entity<Store>,
    entries: Option<advanced::Keys>,
    keymaps: [Option<Keymap>; LAYERS],
    layer: Layer,
    selected: Option<(Index, Layer)>,
    draft: Option<Draft>,
    slot: Option<ActionSlot>,
    picker: Entity<ActionPicker>,
    hold_time: Entity<SliderState>,
    bottom_out: Entity<SliderState>,
    revision: u64,
    seen: Instant,
    tester: Option<tester::Tester>,
    tester_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
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

impl AdvancedPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| {
            ActionPicker::new(
                "Choose an action slot of the advanced key to bind an action to it",
                "No action",
                window,
                cx,
            )
        });
        let hold_time = cx.new(|_| {
            SliderState::new()
                .min(HOLD_TIME_MIN_MS)
                .max(HOLD_TIME_MAX_MS)
                .step(HOLD_TIME_STEP_MS)
                .default_value(f32::from(advanced::ModTap::DEFAULT_HOLD_TIME_MS))
        });
        let bottom_out = cx.new(|_| {
            SliderState::new()
                .min(BOTTOM_OUT_MIN_MM)
                .max(BOTTOM_OUT_MAX_MM)
                .step(BOTTOM_OUT_STEP_MM)
                .default_value(BOTTOM_OUT_MAX_MM * DEFAULT_BOTTOM_OUT_SHARE)
        });
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.load(window, cx);
                }
                StoreEvent::Disconnected => {
                    this.entries = None;
                    this.keymaps = [const { None }; LAYERS];
                    this.deselect();
                    this.draft = None;
                    this.sync_picker(cx);
                    cx.notify();
                }
                StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &PickerEvent, window, cx| {
                    let PickerEvent::Picked(action) = event;
                    this.set_slot_action(*action, window, cx);
                },
            ),
            cx.subscribe_in(
                &hold_time,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    if let SliderEvent::Release(value) = event {
                        let milliseconds = milliseconds_of(value.end());
                        this.change_selected(
                            |entry| {
                                if let Behaviour::ModTap(mod_tap) = &mut entry.behaviour {
                                    mod_tap.hold_time_ms = milliseconds;
                                }
                            },
                            window,
                            cx,
                        );
                    }
                },
            ),
            cx.subscribe_in(
                &bottom_out,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    if let SliderEvent::Release(value) = event {
                        this.set_bottom_out(Some(value.end()), window, cx);
                    }
                },
            ),
        ];
        let mut page = Self {
            store,
            entries: None,
            keymaps: [const { None }; LAYERS],
            layer: Layer::Base,
            selected: None,
            draft: None,
            slot: None,
            picker,
            hold_time,
            bottom_out,
            revision: 0,
            seen: Instant::now(),
            tester: None,
            tester_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let profile = keyboard.active;
        self.picker
            .update(cx, |picker, cx| picker.set_keyboard(&keyboard, cx));
        let entries = self.store.update(cx, |store, cx| {
            store.fetch::<GetAdvancedKeys, _>(profile, cx)
        });
        let keymaps = Layer::ALL.map(|layer| {
            self.store.update(cx, |store, cx| {
                store.fetch::<GetKeymap, _>(LayerRef { profile, layer }, cx)
            })
        });
        let revision = self.revision;
        cx.spawn_in(window, async move |this, cx| {
            let entries = entries.await.ok();
            let mut loaded = [const { None }; LAYERS];
            for (slot, keymap) in loaded.iter_mut().zip(keymaps) {
                *slot = keymap.await.ok();
            }
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    page.load(window, cx);
                    return;
                }
                page.entries = entries;
                page.keymaps = loaded;
                if page.selected_index().is_none() {
                    page.deselect();
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
                if page.selected_index().is_none() {
                    page.deselect();
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

    fn selected_index(&self) -> Option<usize> {
        let (key, layer) = self.selected?;
        self.entries()
            .iter()
            .position(|entry| entry.key == key && entry.layer == layer)
    }

    fn selected_entry(&self) -> Option<&advanced::Key> {
        self.selected_index()
            .and_then(|index| self.entries().get(index))
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

    fn index_name(&self, index: Index) -> String {
        self.key_name(usize::from(index.get()))
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let target = self
            .selected_entry()
            .zip(self.slot)
            .and_then(|(entry, slot)| {
                slot_action(entry, slot).map(|current| Target {
                    layer: entry.layer,
                    current: Some(current),
                })
            });
        self.picker
            .update(cx, |picker, cx| picker.set_target(target, cx));
    }

    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry().copied() else {
            return;
        };
        match entry.behaviour {
            Behaviour::ModTap(mod_tap) => {
                let milliseconds = f32::from(mod_tap.hold_time_ms);
                self.hold_time
                    .update(cx, |slider, cx| slider.set_value(milliseconds, window, cx));
            }
            Behaviour::DynamicKeystroke(keystroke) => {
                let Some(keyboard) = self.store.read(cx).keyboard() else {
                    return;
                };
                let key = usize::from(entry.key.get());
                let millimetres = keystroke
                    .bottom_out
                    .map_or_else(
                        || keyboard.travel_mm(key) * DEFAULT_BOTTOM_OUT_SHARE,
                        |distance| keyboard.distance_mm(key, distance),
                    )
                    .clamp(BOTTOM_OUT_MIN_MM, BOTTOM_OUT_MAX_MM);
                self.bottom_out
                    .update(cx, |slider, cx| slider.set_value(millimetres, window, cx));
            }
            Behaviour::Toggle(_) | Behaviour::DepthPriority(_) | Behaviour::Socd(_) => {}
        }
    }

    fn choose_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        if self.layer != layer {
            self.layer = layer;
            self.draft = None;
            if self.selected.is_some_and(|(_, selected)| selected != layer) {
                self.deselect();
                self.sync_picker(cx);
            }
        }
        cx.notify();
    }

    fn deselect(&mut self) {
        self.selected = None;
        self.slot = None;
        self.tester = None;
    }

    fn select(&mut self, key: Index, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != Some((key, layer)) {
            self.tester = None;
        }
        self.layer = layer;
        self.draft = None;
        self.selected = Some((key, layer));
        self.slot = self
            .selected_entry()
            .and_then(|entry| first_slot(&entry.behaviour));
        self.sync_sliders(window, cx);
        self.sync_picker(cx);
        cx.notify();
    }

    fn choose_slot(&mut self, slot: ActionSlot, cx: &mut Context<Self>) {
        self.slot = Some(slot);
        self.sync_picker(cx);
        cx.notify();
    }

    fn click_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        let Some(index) = u8::try_from(key).ok().map(Index::new) else {
            return;
        };
        let holder = rules::holder(self.entries(), index, layer);
        if let Some(draft) = &mut self.draft {
            if draft.keys.contains(&key) {
                draft.keys.retain(|chosen| *chosen != key);
            } else if holder.is_some() {
                window.push_notification(
                    Notification::warning(format!(
                        "This key already has an advanced key on {}",
                        layer_phrase(layer)
                    )),
                    cx,
                );
            } else {
                if draft.keys.len() >= draft.kind.key_count() {
                    draft.keys.pop();
                }
                draft.keys.push(key);
            }
            cx.notify();
            return;
        }
        if let Some(primary) = holder
            .and_then(|position| self.entries().get(position))
            .map(|entry| entry.key)
        {
            self.select(primary, layer, window, cx);
        } else {
            self.deselect();
            self.sync_picker(cx);
            cx.notify();
        }
    }

    fn start_draft(&mut self, kind: Kind, cx: &mut Context<Self>) {
        self.draft = Some(Draft {
            kind,
            keys: Vec::new(),
        });
        self.deselect();
        self.sync_picker(cx);
        cx.notify();
    }

    fn cancel_draft(&mut self, cx: &mut Context<Self>) {
        self.draft = None;
        cx.notify();
    }

    fn assign_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.clone() else {
            return;
        };
        let keys: Vec<Index> = draft
            .keys
            .iter()
            .filter_map(|key| u8::try_from(*key).ok().map(Index::new))
            .collect();
        let (Some(first), Some(key)) = (keys.first().copied(), draft.keys.first().copied()) else {
            return;
        };
        let Some(behaviour) = draft.kind.behaviour(self.action_of(self.layer, key), &keys) else {
            return;
        };
        let mut entries = self.entries().to_vec();
        entries.push(advanced::Key {
            key: first,
            layer: self.layer,
            behaviour,
        });
        if self.write(&entries, window, cx) {
            self.draft = None;
            self.select(first, self.layer, window, cx);
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
        self.entries = Some(value.clone());
        self.revision += 1;
        let applied = self.store.update(cx, |store, cx| {
            store.apply::<SetAdvancedKeys>(Change { profile, value }, Scope::Profile(profile), cx)
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

    fn change_selected(
        &mut self,
        change: impl FnOnce(&mut advanced::Key),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.selected_index() else {
            return;
        };
        let mut entries = self.entries().to_vec();
        let Some(entry) = entries.get_mut(index) else {
            return;
        };
        let before = *entry;
        change(entry);
        if *entry != before && !self.write(&entries, window, cx) {
            self.sync_sliders(window, cx);
        }
    }

    fn set_slot_action(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slot) = self.slot else {
            return;
        };
        self.change_selected(
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
        self.change_selected(
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
    }

    fn delete(&mut self, key: Index, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let entries: Vec<advanced::Key> = self
            .entries()
            .iter()
            .filter(|entry| !(entry.key == key && entry.layer == layer))
            .copied()
            .collect();
        if self.selected == Some((key, layer)) {
            self.deselect();
        }
        self.write(&entries, window, cx);
    }

    fn confirm_delete(entry: advanced::Key, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let title = Kind::of(&entry.behaviour).title();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(format!("Remove this {title} binding?"))
                .description("Are you sure you want to remove this advanced key?")
                .confirm()
                .ok_text("Remove")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| {
                        page.delete(entry.key, entry.layer, window, cx);
                    })
                    .ok();
                    true
                })
        });
    }

    fn preset_plan(
        &self,
        preset: &Preset,
    ) -> Result<(Vec<advanced::Key>, Vec<advanced::Key>), String> {
        let Some(base) = self.keymaps.first().and_then(Option::as_ref) else {
            return Err("Reading the base layer…".to_owned());
        };
        let added = preset.entries(base).map_err(|missing| {
            let names: Vec<String> = missing
                .iter()
                .map(|action| usage::name(*action).into_owned())
                .collect();
            format!(
                "Needs a key bound to {} on the base layer",
                names.join(", ")
            )
        })?;
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
        if total > MAX_ENTRIES {
            return Err(format!(
                "This preset would exceed your Advanced Keys limit. Remove {} binding(s) to apply.",
                total - MAX_ENTRIES
            ));
        }
        Ok((added, replaced))
    }

    fn apply_preset(&mut self, preset: &Preset, window: &mut Window, cx: &mut Context<Self>) {
        let (added, replaced) = match self.preset_plan(preset) {
            Ok(plan) => plan,
            Err(reason) => {
                window.push_notification(Notification::warning(reason), cx);
                return;
            }
        };
        let mut entries: Vec<advanced::Key> = self
            .entries()
            .iter()
            .filter(|entry| !replaced.contains(entry))
            .copied()
            .collect();
        entries.extend(added.iter().copied());
        if self.write(&entries, window, cx)
            && let Some(first) = added.first()
        {
            self.select(first.key, first.layer, window, cx);
        }
    }

    fn confirm_preset(
        &mut self,
        preset: &'static Preset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (added, replaced) = match self.preset_plan(preset) {
            Ok(plan) => plan,
            Err(reason) => {
                window.push_notification(Notification::warning(reason), cx);
                return;
            }
        };
        if replaced.is_empty() {
            self.apply_preset(preset, window, cx);
            return;
        }
        let page = cx.entity().downgrade();
        let describe = |entry: &advanced::Key| {
            let keys: Vec<String> = rules::bindings(entry)
                .map(|(key, _)| self.index_name(key))
                .collect();
            format!(
                "{} on {}",
                Kind::of(&entry.behaviour).title(),
                keys.join(" + ")
            )
        };
        let current: Vec<String> = replaced.iter().map(describe).collect();
        let new: Vec<String> = added.iter().map(describe).collect();
        let count = replaced.len();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(format!(
                    "This preset will replace {count} existing Advanced Key{}",
                    if count == 1 { "" } else { "s" }
                ))
                .description(format!(
                    "Current: {}. New: {}.",
                    current.join("; "),
                    new.join("; ")
                ))
                .confirm()
                .ok_text("Apply preset")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.apply_preset(preset, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn face(&self, keyboard: &Keyboard, key: usize, look: Look) -> KeyFace {
        let layer = self.layer;
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
        let mut face = KeyFace {
            legend: usage::legend(match own {
                Some(Action::None) | None => base,
                Some(action) => action,
            })
            .into(),
            dimmed: matches!(own, Some(Action::None) | None),
            ..KeyFace::default()
        };
        let holder = u8::try_from(key)
            .ok()
            .and_then(|key| rules::holder(self.entries(), Index::new(key), layer))
            .and_then(|position| self.entries().get(position));
        if let Some(entry) = holder {
            face.detail = Some(Kind::of(&entry.behaviour).badge().into());
            face.fill = Some(if self.draft.is_some() {
                look.taken
            } else {
                look.bound
            });
            face.dimmed = false;
        }
        face.travel = self.tested_travel(key);
        face
    }

    fn view_selection(&self) -> Selection {
        let mut selection = Selection::default();
        if let Some(draft) = &self.draft {
            selection.set(draft.keys.iter().copied());
        } else if let Some(entry) = self.selected_entry() {
            selection.set(rules::bindings(entry).map(|(key, _)| usize::from(key.get())));
        }
        selection
    }

    fn layer_bar(&self, cx: &mut Context<Self>) -> Div {
        let used = self.entries().len();
        h_flex()
            .w_full()
            .gap_3()
            .items_center()
            .child(
                TabBar::new("advanced-layers")
                    .segmented()
                    .selected_index(usize::from(self.layer.number()))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(layer) = Layer::ALL.get(*index) {
                            this.choose_layer(*layer, cx);
                        }
                    }))
                    .children(Layer::ALL.map(|layer| {
                        let count = self
                            .entries()
                            .iter()
                            .filter(|entry| entry.layer == layer)
                            .count();
                        let title = if count == 0 {
                            usage::layer_name(layer).to_owned()
                        } else {
                            format!("{} ({count})", usage::layer_name(layer))
                        };
                        Tab::new().label(title)
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(format!(
                        "{used} of {MAX_ENTRIES} advanced keys used"
                    ))),
            )
    }

    fn keyboard_section(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let look = Look {
            bound: theme.primary.opacity(0.3),
            taken: theme.danger.opacity(0.3),
        };
        let page = cx.entity().downgrade();
        let board = KeyboardView::new("advanced-keyboard", keyboard, |key| {
            self.face(keyboard, key, look)
        })
        .selection(&self.view_selection())
        .fit_width(window.viewport_size().width - px(KEYBOARD_MARGIN))
        .on_key_click(move |key, _, window, cx| {
            page.update(cx, |page, cx| page.click_key(key, window, cx))
                .ok();
        });
        let section = panel::section(
            usage::layer_name(self.layer),
            Some(
                "Keys with an advanced key on this layer are highlighted. Click one to edit it."
                    .into(),
            ),
            cx,
        )
        .child(div().w_full().overflow_hidden().child(board));
        match &self.draft {
            Some(draft) => section.child(self.draft_bar(draft, cx)),
            None => section,
        }
    }

    fn draft_bar(&self, draft: &Draft, cx: &mut Context<Self>) -> Div {
        let needed = draft.kind.key_count();
        let prompt = if needed == 1 {
            format!(
                "Select a key from your keyboard to assign to {}",
                draft.kind.title()
            )
        } else {
            format!(
                "Select {needed} keys from your keyboard to assign {} ({} of {needed} selected)",
                draft.kind.title(),
                draft.keys.len()
            )
        };
        let chosen: Vec<String> = draft.keys.iter().map(|key| self.key_name(*key)).collect();
        h_flex()
            .gap_2()
            .flex_wrap()
            .items_center()
            .p_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().primary)
            .child(
                v_flex()
                    .flex_1()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(prompt),
                    )
                    .when(!chosen.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(SharedString::from(chosen.join(" + "))),
                        )
                    }),
            )
            .child(
                Button::new("draft-cancel")
                    .small()
                    .ghost()
                    .label("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_draft(cx))),
            )
            .child(
                Button::new("draft-assign")
                    .small()
                    .primary()
                    .label("Assign")
                    .disabled(draft.keys.len() != needed)
                    .on_click(cx.listener(|this, _, window, cx| this.assign_draft(window, cx))),
            )
    }

    fn list_section(&self, cx: &mut Context<Self>) -> Div {
        let mut section = panel::section(
            "Active Advanced Keys",
            Some("There are a limited number of slots per profile.".into()),
            cx,
        );
        if self.entries().is_empty() {
            return section.child(panel::note(
                "No Advanced Keys configured yet. Select one from the list to add it.",
                cx,
            ));
        }
        for layer in Layer::ALL {
            let rows: Vec<Div> = self
                .entries()
                .iter()
                .filter(|entry| entry.layer == layer)
                .map(|entry| self.entry_row(*entry, cx))
                .collect();
            if rows.is_empty() {
                continue;
            }
            section = section.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child(usage::layer_name(layer)),
                    )
                    .children(rows),
            );
        }
        section
    }

    fn entry_row(&self, entry: advanced::Key, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let kind = Kind::of(&entry.behaviour);
        let selected = self.selected == Some((entry.key, entry.layer));
        let keys: Vec<String> = rules::bindings(&entry)
            .map(|(key, _)| self.index_name(key))
            .collect();
        let id = format!("entry-{}-{}", entry.layer.number(), entry.key.get());
        h_flex()
            .gap_2()
            .items_center()
            .child(
                h_flex()
                    .id(SharedString::from(format!("{id}-row")))
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(if selected {
                        theme.primary
                    } else {
                        theme.border
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.secondary))
                    .child(
                        div()
                            .w_32()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(SharedString::from(keys.join(" + "))),
                    )
                    .child(div().w_40().text_sm().child(kind.title()))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(self.summary(&entry))),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select(entry.key, entry.layer, window, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("{id}-delete")))
                    .small()
                    .ghost()
                    .icon(Catalog::Trash)
                    .tooltip("Delete")
                    .on_click(cx.listener(move |_, _, window, cx| {
                        Self::confirm_delete(entry, window, cx);
                    })),
            )
    }

    fn summary(&self, entry: &advanced::Key) -> String {
        match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => {
                let bound: Vec<String> = keystroke
                    .strokes
                    .iter()
                    .filter(|stroke| stroke.action != Action::None)
                    .map(|stroke| usage::name(stroke.action).into_owned())
                    .collect();
                if bound.is_empty() {
                    "No bindings yet".to_owned()
                } else {
                    bound.join(", ")
                }
            }
            Behaviour::ModTap(mod_tap) => format!(
                "Tap: {} · Hold: {} · {} ms",
                usage::name(mod_tap.tap),
                usage::name(mod_tap.hold),
                mod_tap.hold_time_ms
            ),
            Behaviour::Toggle(action) => usage::name(action).into_owned(),
            Behaviour::DepthPriority(pair) => {
                let first = self.index_name(entry.key);
                let second = self.index_name(pair.other);
                kind::bottomed_out_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| choice.value == pair.both_bottomed_out)
                    .map_or_else(String::new, |choice| {
                        format!("Both fully pressed: {}", choice.title)
                    })
            }
            Behaviour::Socd(pair) => {
                let first = self.index_name(entry.key);
                let second = self.index_name(pair.other);
                let title = kind::socd_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| choice.value == pair.resolution)
                    .map_or_else(String::new, |choice| choice.title);
                if pair.both_when_bottomed_out {
                    format!("{title} · both when fully pressed")
                } else {
                    title
                }
            }
        }
    }

    fn create_section(&self, cx: &mut Context<Self>) -> Div {
        let full = self.entries().len() >= MAX_ENTRIES;
        let loaded = self.entries.is_some() && self.keymaps.iter().all(Option::is_some);
        let theme = cx.theme();
        let cards = Kind::ALL.map(|kind| {
            let drafting = self.draft.as_ref().is_some_and(|draft| draft.kind == kind);
            v_flex()
                .w(px(260.0))
                .gap_2()
                .p_3()
                .rounded(theme.radius)
                .border_1()
                .border_color(if drafting {
                    theme.primary
                } else {
                    theme.border
                })
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(kind.title()),
                )
                .child(
                    div()
                        .flex_1()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(kind.description()),
                )
                .child(
                    Button::new(SharedString::from(format!("create-{kind:?}")))
                        .small()
                        .outline()
                        .icon(Catalog::Plus)
                        .label("Add")
                        .disabled(full || !loaded)
                        .on_click(cx.listener(move |this, _, _, cx| this.start_draft(kind, cx))),
                )
        });
        let description: SharedString = if full {
            "You have reached the maximum amount of Advanced Keys for this profile.".into()
        } else {
            format!(
                "To create a binding, first select a type of advanced key below, then select its \
                 keys on the keyboard. It goes on {}.",
                usage::layer_name(self.layer)
            )
            .into()
        };
        panel::section("Add Advanced Key", Some(description), cx)
            .child(h_flex().gap_3().flex_wrap().items_start().children(cards))
    }

    fn presets_section(&self, cx: &mut Context<Self>) -> Div {
        let section = panel::section(
            "Preset bindings",
            Some(
                "Select a preset to instantly apply common advanced key configurations. You can \
                 always customise it afterwards."
                    .into(),
            ),
            cx,
        );
        if self.layer != Layer::Base {
            return section.child(panel::note(
                "Only available on the base layer. Switch layers to apply.",
                cx,
            ));
        }
        let theme = cx.theme();
        let rows: Vec<Div> = PRESETS
            .iter()
            .enumerate()
            .map(|(position, preset)| {
                let plan = self.preset_plan(preset);
                let unavailable = plan.as_ref().err().cloned();
                h_flex()
                    .gap_3()
                    .items_center()
                    .py_1()
                    .child(
                        div()
                            .w_32()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(preset.kind.title()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(preset.title),
                            )
                            .child(
                                div().text_xs().text_color(theme.muted_foreground).child(
                                    unavailable
                                        .clone()
                                        .unwrap_or_else(|| preset.description.to_owned()),
                                ),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("preset-{position}")))
                            .small()
                            .outline()
                            .label("Apply")
                            .disabled(unavailable.is_some())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(preset) = PRESETS.get(position) {
                                    this.confirm_preset(preset, window, cx);
                                }
                            })),
                    )
            })
            .collect();
        section.children(rows)
    }
}

impl Render for AdvancedPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page(
            "Advanced keys",
            "Advanced keys change the way your keyboard processes a keypress. They are powerful \
             tools that can enhance gameplay and improve productivity.",
            cx,
        );
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_keymaps(cx);
        }
        self.seen = Instant::now();
        let editor = self.editor_section(&keyboard, cx);
        page.child(self.layer_bar(cx))
            .child(self.keyboard_section(&keyboard, window, cx))
            .children(editor)
            .child(self.list_section(cx))
            .child(self.create_section(cx))
            .child(self.presets_section(cx))
    }
}

#[derive(Debug, Clone, Copy)]
struct Look {
    bound: Hsla,
    taken: Hsla,
}

const fn first_slot(behaviour: &Behaviour) -> Option<ActionSlot> {
    match behaviour {
        Behaviour::DynamicKeystroke(_) => Some(ActionSlot::Stroke(0)),
        Behaviour::ModTap(_) => Some(ActionSlot::Tap),
        Behaviour::Toggle(_) => Some(ActionSlot::Toggle),
        Behaviour::DepthPriority(_) | Behaviour::Socd(_) => None,
    }
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
        Layer::Base => "the base layer",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_durations_stay_within_the_slider_range() {
        assert_eq!(milliseconds_of(204.6), 205);
        assert_eq!(milliseconds_of(0.0), 10);
        assert_eq!(milliseconds_of(5000.0), 1000);
    }
}
