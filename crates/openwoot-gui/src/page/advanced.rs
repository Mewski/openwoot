mod editor;
mod kind;
mod preset;
mod rules;
mod tester;

use std::time::{Duration, Instant};

use gpui_kit::{
    App, AppContext as _, Context, Div, Entity, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        notification::Notification,
        slider::{SliderEvent, SliderState},
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
use super::{
    Page,
    remap::{ActionPicker, PickerEvent, Target, caption, layer_label, layer_pill, layers_column},
};
use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const HOLD_TIME_MIN_MS: f32 = 10.0;
const HOLD_TIME_MAX_MS: f32 = 1000.0;
const HOLD_TIME_STEP_MS: f32 = 10.0;
const BOTTOM_OUT_MIN_MM: f32 = 0.1;
const BOTTOM_OUT_MAX_MM: f32 = 4.0;
const BOTTOM_OUT_STEP_MM: f32 = 0.05;
const DEFAULT_BOTTOM_OUT_SHARE: f32 = 0.9;
const STALE_AFTER: Duration = Duration::from_secs(2);
const ROW_HEIGHT: f32 = 40.0;
const GLYPH_SIZE: f32 = 44.0;
const UNAVAILABLE_OPACITY: f32 = 0.5;
const ADD_ORDER: [Kind; 5] = [
    Kind::DepthPriority,
    Kind::Socd,
    Kind::DynamicKeystroke,
    Kind::ModTap,
    Kind::Toggle,
];

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
        let picker = cx.new(|cx| ActionPicker::new("None", window, cx));
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
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
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
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(_) => cx.notify(),
                    SliderEvent::Release(value) => {
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

    fn keys_name(&self, entry: &advanced::Key) -> String {
        let names: Vec<String> = rules::bindings(entry)
            .map(|(key, _)| self.index_name(key))
            .collect();
        names.join(" + ")
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

    fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.deselect();
        self.draft = None;
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
                let name = self.key_name(key);
                window.push_notification(
                    Notification::warning(format!(
                        "{name} already has an advanced key on this layer"
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
            self.close_editor(cx);
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

    fn confirm_delete(&self, entry: advanced::Key, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let title = format!(
            "Delete {} on {}?",
            Kind::of(&entry.behaviour).title(),
            self.keys_name(&entry)
        );
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(title.clone())
                .confirm()
                .ok_text("Delete")
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
                    Kind::of(&entry.behaviour).title(),
                    self.keys_name(entry)
                )
            })
            .collect();
        let count = replaced.len();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(if count == 1 {
                    "Replace 1 advanced key?".to_owned()
                } else {
                    format!("Replace {count} advanced keys?")
                })
                .description(format!("{}.", current.join(", ")))
                .confirm()
                .ok_text("Replace")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.apply_preset(preset, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn face(&self, keyboard: &Keyboard, key: usize) -> KeyFace {
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
            face.dimmed = self.draft.is_some();
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

    fn layers(&self, cx: &mut Context<Self>) -> Div {
        let pills: Vec<Stateful<Div>> = Layer::ALL
            .into_iter()
            .map(|layer| {
                layer_pill("advanced", layer, self.layer == layer, None, cx)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_layer(layer, cx)))
            })
            .collect();
        layers_column(pills, cx)
    }

    fn active_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let count = div()
            .absolute()
            .top_6()
            .right_6()
            .px_2()
            .py_0p5()
            .rounded(theme.radius)
            .bg(theme.secondary)
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .child(SharedString::from(format!(
                "{} / {MAX_ENTRIES}",
                self.entries().len()
            )));
        let card = panel::section("Active advanced keys", None, cx)
            .relative()
            .child(count);
        if self.entries().is_empty() {
            return card.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No advanced keys yet"),
            );
        }
        let mut groups = v_flex().gap_4();
        for layer in Layer::ALL {
            let rows: Vec<Stateful<Div>> = self
                .entries()
                .iter()
                .filter(|entry| entry.layer == layer)
                .map(|entry| self.entry_row(*entry, cx))
                .collect();
            if rows.is_empty() {
                continue;
            }
            groups = groups.child(
                v_flex()
                    .gap_2()
                    .child(panel::label(layer_label(layer), cx))
                    .children(rows),
            );
        }
        card.child(groups)
    }

    fn entry_row(&self, entry: advanced::Key, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (muted, fill, hover, accent, radius) = (
            theme.muted_foreground,
            theme.accent,
            theme.secondary,
            theme.primary,
            theme.radius,
        );
        let selected = self.selected == Some((entry.key, entry.layer));
        let id = format!("entry-{}-{}", entry.layer.number(), entry.key.get());
        h_flex()
            .id(SharedString::from(format!("{id}-row")))
            .w_full()
            .h(px(ROW_HEIGHT))
            .pl_3()
            .pr_1()
            .gap_3()
            .items_center()
            .rounded(radius)
            .border_1()
            .border_color(if selected { accent } else { fill })
            .bg(fill)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(self.keys_name(&entry))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(Kind::of(&entry.behaviour).title()),
            )
            .child(
                Button::new(SharedString::from(format!("{id}-delete")))
                    .ghost()
                    .xsmall()
                    .icon(Catalog::Trash)
                    .tooltip("Delete")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.confirm_delete(entry, window, cx);
                    })),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(entry.key, entry.layer, window, cx);
            }))
    }

    fn presets_card(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.layer != Layer::Base || self.selected.is_some() || self.draft.is_some() {
            return None;
        }
        let muted = cx.theme().muted_foreground;
        let mut groups = v_flex().gap_4();
        let mut any = false;
        for kind in Kind::ALL {
            let rows: Vec<Div> = PRESETS
                .iter()
                .enumerate()
                .filter(|(_, preset)| preset.kind == kind)
                .filter_map(|(position, preset)| {
                    let (added, replaced) = self.preset_plan(preset)?;
                    let applied = added.iter().all(|entry| replaced.contains(entry));
                    let control = if applied {
                        div().text_sm().text_color(muted).child("Applied")
                    } else {
                        div().child(
                            Button::new(SharedString::from(format!("preset-{position}")))
                                .small()
                                .label("Apply")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if let Some(preset) = PRESETS.get(position) {
                                        this.confirm_preset(preset, window, cx);
                                    }
                                })),
                        )
                    };
                    Some(panel::row(
                        preset.title,
                        Some(preset.description.into()),
                        control,
                        cx,
                    ))
                })
                .collect();
            if rows.is_empty() {
                continue;
            }
            any = true;
            groups = groups.child(
                v_flex()
                    .gap_1()
                    .child(panel::label(kind.title(), cx))
                    .children(rows),
            );
        }
        any.then(|| panel::section("Presets", None, cx).child(groups))
    }

    fn add_card(&self, cx: &mut Context<Self>) -> Div {
        let available = self.entries().len() < MAX_ENTRIES
            && self.entries.is_some()
            && self.keymaps.iter().all(Option::is_some);
        let tiles: Vec<Stateful<Div>> = ADD_ORDER
            .into_iter()
            .map(|kind| Self::kind_tile(kind, available, cx))
            .collect();
        panel::section(
            "Add advanced key",
            Some("Choose a type, then the keys it applies to".into()),
            cx,
        )
        .child(v_flex().gap_2().children(tiles))
    }

    fn kind_tile(kind: Kind, available: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (fill, hover, border, muted, radius) = (
            theme.accent,
            theme.secondary,
            theme.border,
            theme.muted_foreground,
            theme.radius,
        );
        let tile = h_flex()
            .id(SharedString::from(format!("add-{kind:?}")))
            .w_full()
            .gap_4()
            .px_4()
            .py_3()
            .items_center()
            .rounded(radius)
            .bg(fill)
            .child(
                h_flex()
                    .flex_none()
                    .size(px(GLYPH_SIZE))
                    .items_center()
                    .justify_center()
                    .rounded(radius)
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(kind.badge()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(kind.title()),
                    )
                    .child(div().text_sm().text_color(muted).child(kind.help())),
            );
        if available {
            tile.cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _, _, cx| this.start_draft(kind, cx)))
        } else {
            tile.opacity(UNAVAILABLE_OPACITY)
        }
    }

    fn draft_card(&self, draft: &Draft, cx: &mut Context<Self>) -> Div {
        let needed = draft.kind.key_count();
        let fields: Vec<Div> = (0..needed)
            .map(|position| {
                let label = match (needed, position) {
                    (1, _) => "Key",
                    (_, 0) => "First key",
                    _ => "Second key",
                };
                let chosen = draft.keys.get(position).map(|key| self.key_name(*key));
                div()
                    .flex_1()
                    .child(panel::field(label, value_box(chosen, cx), cx))
            })
            .collect();
        panel::section(draft.kind.title(), Some(draft.kind.help().into()), cx)
            .child(h_flex().w_full().gap_4().children(fields))
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("draft-cancel")
                            .ghost()
                            .small()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
                    )
                    .child(
                        Button::new("draft-create")
                            .primary()
                            .small()
                            .label("Create")
                            .disabled(draft.keys.len() != needed)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.assign_draft(window, cx)),
                            ),
                    ),
            )
    }
}

impl Render for AdvancedPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_keymaps(cx);
        }
        self.seen = Instant::now();
        let weak = cx.entity().downgrade();
        let view = KeyboardView::new("advanced-keyboard", &keyboard, |key| {
            self.face(&keyboard, key)
        })
        .selection(&self.view_selection())
        .fit_width(panel::content_width_beside_side(window))
        .on_key_click(move |key, _, window, cx| {
            weak.update(cx, |page, cx| page.click_key(key, window, cx))
                .ok();
        });
        let caption = self
            .draft
            .as_ref()
            .filter(|draft| draft.keys.len() < draft.kind.key_count())
            .map(|draft| {
                caption(
                    if draft.kind.key_count() == 1 {
                        "Select a key"
                    } else {
                        "Select two keys"
                    },
                    cx,
                )
            });
        let (left, right) = if let Some(draft) = &self.draft {
            (self.draft_card(draft, cx), self.active_card(cx))
        } else if let Some(editor) = self.editor_card(&keyboard, cx) {
            let picks = self
                .selected_entry()
                .is_some_and(|entry| Kind::of(&entry.behaviour).owns_actions());
            let right = if picks {
                panel::section("Actions", None, cx).child(self.picker.clone())
            } else {
                self.active_card(cx)
            };
            (editor, right)
        } else {
            let right = v_flex()
                .gap_4()
                .child(self.active_card(cx))
                .children(self.presets_card(cx));
            (self.add_card(cx), right)
        };
        page.child(panel::stage_with_side(self.layers(cx), view))
            .children(caption)
            .child(panel::heading(
                Page::Advanced.title(),
                Some("Give keys extra behaviour, such as tap and hold or several actions".into()),
                div(),
                cx,
            ))
            .child(panel::columns(left, right))
    }
}

fn value_box(value: Option<String>, cx: &App) -> Div {
    let theme = cx.theme();
    let empty = value.is_none();
    h_flex()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px_3()
        .items_center()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.input)
        .text_sm()
        .when(empty, |this| this.text_color(theme.muted_foreground))
        .child(SharedString::from(value.unwrap_or_else(|| "—".to_owned())))
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
