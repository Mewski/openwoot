mod catalog;
mod factory;
mod keycap;
mod layers;
mod letter_layout;
mod picker;
mod quick_bind;

use std::time::{Duration, Instant};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, notification::Notification,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour},
    configuration::Validate as _,
    endpoints::{GetAdvancedKeys, GetDefaultKeymap, GetKeymap, SetKeymap},
    keys::{Action, Function, Keymap, LAYERS, Layer},
    profile::{LayerChange, LayerRef},
};

use self::{
    factory::{Defaults, Factory},
    letter_layout::LetterLayout,
};
pub(super) use self::{
    keycap::{blank_chip, chip, face as keycap_face, glyph},
    layers::{column as layers_column, layer_label, pill as layer_pill},
    picker::{ActionPicker, PickerEvent, Target},
    quick_bind::usage_of_key,
};
use super::{Page, advanced::Kind};
use crate::{
    dropdown::{Dropdown, Item},
    keyboard::{Dragged, KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    theme, usage,
};

const STALE_AFTER: Duration = Duration::from_secs(2);
const SIDE_CARD_WIDTH: f32 = 340.0;
const LAYOUT_TILE_SIZE: f32 = 68.0;
const MENU_WIDTH: f32 = 240.0;
const BIND_PROMPT: &str = "Press any key to bind";
const MANAGED: &str =
    "This key has an advanced key configured. To remap it, remove the advanced key first.";
const LOCKED: &str = "This layer switch key is configured from the main layer";
const UNBOUND: &str = "This key won't register any input";
const UNREACHABLE: &str = "No key opens this layer";
const REMAP_HELP: &str =
    "Change what each key sends, on the main layer and on Fn layers reached by holding a Fn key";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Moved(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayerCommand {
    Copy,
    PasteReplace,
    PasteMerge,
    Duplicate,
    Delete,
}

pub(crate) struct RemapPage {
    store: Entity<Store>,
    layer: Layer,
    keymaps: [Option<Keymap>; LAYERS],
    advanced: Option<advanced::Keys>,
    defaults: Option<Defaults>,
    selected: Option<usize>,
    picker: Entity<ActionPicker>,
    copied: Option<(Layer, Keymap)>,
    added: [bool; LAYERS],
    focus: FocusHandle,
    revision: u64,
    seen: Instant,
    _subscriptions: Vec<Subscription>,
}

impl LayerCommand {
    const ALL: [Self; 5] = [
        Self::Copy,
        Self::PasteReplace,
        Self::PasteMerge,
        Self::Duplicate,
        Self::Delete,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::PasteReplace => "Paste and replace",
            Self::PasteMerge => "Paste and merge",
            Self::Duplicate => "Duplicate",
            Self::Delete => "Delete",
        }
    }

    const fn shortcut(self) -> Option<&'static str> {
        match self {
            Self::Copy => Some("C"),
            Self::PasteReplace => Some("V"),
            Self::Duplicate => Some("D"),
            Self::PasteMerge | Self::Delete => None,
        }
    }
}

impl RemapPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| ActionPicker::new(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.load(window, cx);
                }
                StoreEvent::Disconnected => {
                    this.keymaps = [const { None }; LAYERS];
                    this.advanced = None;
                    this.deselect(cx);
                    cx.notify();
                }
                StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &PickerEvent, window, cx| match event {
                    PickerEvent::Picked(action) => this.picked(*action, window, cx),
                    PickerEvent::Cancelled => this.deselect(cx),
                },
            ),
        ];
        let mut page = Self {
            store,
            layer: Layer::Base,
            keymaps: [const { None }; LAYERS],
            advanced: None,
            defaults: None,
            selected: None,
            picker,
            copied: None,
            added: [false; LAYERS],
            focus: cx.focus_handle(),
            revision: 0,
            seen: Instant::now(),
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
        let keymaps = Layer::ALL.map(|layer| {
            self.store.update(cx, |store, cx| {
                store.fetch::<GetKeymap, _>(LayerRef { profile, layer }, cx)
            })
        });
        let advanced = self.store.update(cx, |store, cx| {
            store.fetch::<GetAdvancedKeys, _>(profile, cx)
        });
        let defaults = [Layer::Base, Layer::One].map(|layer| {
            self.store.update(cx, |store, cx| {
                store.fetch::<GetDefaultKeymap, _>(layer, cx)
            })
        });
        let revision = self.revision;
        cx.spawn_in(window, async move |this, cx| {
            let mut loaded = [const { None }; LAYERS];
            for (slot, keymap) in loaded.iter_mut().zip(keymaps) {
                *slot = keymap.await.ok();
            }
            let advanced = advanced.await.ok();
            let [main, function] = defaults;
            let defaults = match (main.await, function.await) {
                (Ok(main), Ok(function)) => Some(Defaults { main, function }),
                _ => None,
            };
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    page.load(window, cx);
                    return;
                }
                page.keymaps = loaded;
                page.advanced = advanced;
                page.defaults = defaults;
                if !page.shown(page.layer) {
                    page.layer = Layer::Base;
                }
                page.deselect(cx);
                page.sync_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_advanced(&self, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let call = self
            .store
            .read(cx)
            .device()
            .configure::<GetAdvancedKeys, advanced::Keys>(profile);
        cx.spawn(async move |this, cx| {
            let Ok(advanced) = call.await else {
                return;
            };
            this.update(cx, |page, cx| {
                if page.advanced.as_ref() != Some(&advanced) {
                    page.advanced = Some(advanced);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn reload_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile) = self.store.read(cx).active() else {
            return;
        };
        let keymap = self.store.update(cx, |store, cx| {
            store.fetch::<GetKeymap, _>(LayerRef { profile, layer }, cx)
        });
        cx.spawn_in(window, async move |this, cx| {
            let keymap = keymap.await.ok();
            this.update(cx, |page, cx| {
                if layer == Layer::Base
                    && let Some(keymap) = &keymap
                {
                    let legends = keymap.clone();
                    page.store
                        .update(cx, |store, cx| store.set_legends(legends, cx));
                }
                if let Some(slot) = page.keymaps.get_mut(usize::from(layer.number())) {
                    *slot = keymap;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn keymap(&self, layer: Layer) -> Option<&Keymap> {
        self.keymaps
            .get(usize::from(layer.number()))
            .and_then(Option::as_ref)
    }

    fn action(&self, layer: Layer, key: usize) -> Option<Action> {
        self.keymap(layer)
            .and_then(|keymap| keymap.get(key).copied())
    }

    fn managed(&self, key: usize, layer: Layer) -> Option<Kind> {
        self.advanced
            .iter()
            .flatten()
            .filter(|entry| entry.layer == layer && usize::from(entry.key.get()) == key)
            .map(|entry| Kind::of(&entry.behaviour))
            .find(|kind| kind.owns_actions())
    }

    fn locked(&self, key: usize, layer: Layer) -> bool {
        layer != Layer::Base && self.action(Layer::Base, key) == Some(Action::Layer(layer))
    }

    fn editable(&self, key: usize) -> bool {
        self.keymap(self.layer).is_some()
            && self.managed(key, self.layer).is_none()
            && !self.locked(key, self.layer)
    }

    fn opens(&self, layer: Layer) -> bool {
        let opener = Action::Layer(layer);
        let in_keymaps = Layer::ALL
            .into_iter()
            .filter(|other| *other != layer)
            .filter_map(|other| self.keymap(other))
            .any(|keymap| keymap.contains(&opener));
        let in_advanced = self
            .advanced
            .iter()
            .flatten()
            .any(|entry| match entry.behaviour {
                Behaviour::DynamicKeystroke(keystroke) => keystroke
                    .strokes
                    .iter()
                    .any(|stroke| stroke.action == opener),
                Behaviour::ModTap(mod_tap) => mod_tap.tap == opener || mod_tap.hold == opener,
                Behaviour::Toggle(action) => action == opener,
                Behaviour::DepthPriority(_) | Behaviour::Socd(_) => false,
            });
        in_keymaps || in_advanced
    }

    fn shown(&self, layer: Layer) -> bool {
        layer == Layer::Base
            || self.added.get(usize::from(layer.number())).copied() == Some(true)
            || self
                .keymap(layer)
                .is_some_and(|keymap| keymap.iter().any(|action| *action != Action::None))
            || self.opens(layer)
    }

    fn next_free(&self) -> Option<Layer> {
        Layer::ALL
            .into_iter()
            .skip(1)
            .find(|layer| !self.shown(*layer))
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let layer = self.layer;
        let target = self.selected.map(|key| Target {
            current: self.action(layer, key).unwrap_or_default(),
        });
        self.picker.update(cx, |picker, cx| {
            picker.set_layer(layer, cx);
            picker.set_target(target, cx);
        });
    }

    fn choose_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        self.layer = layer;
        self.deselect(cx);
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn click_key(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable(key) {
            return;
        }
        if self.selected == Some(key) {
            self.deselect(cx);
            return;
        }
        self.selected = Some(key);
        self.sync_picker(cx);
        self.picker
            .update(cx, |picker, cx| picker.listen(window, cx));
        cx.notify();
    }

    fn deselect(&mut self, cx: &mut Context<Self>) {
        self.selected = None;
        self.sync_picker(cx);
        self.picker.update(cx, ActionPicker::stop_listening);
        cx.notify();
    }

    fn picked(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.selected else {
            return;
        };
        self.deselect(cx);
        self.assign(key, action, window, cx);
    }

    fn assign(&mut self, key: usize, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        if !self.editable(key) {
            return;
        }
        if let Some(reason) = self.picker.read(cx).refusal(action) {
            window.push_notification(Notification::warning(reason), cx);
            return;
        }
        let Some(mut keymap) = self.keymap(layer).cloned() else {
            return;
        };
        if let Some(slot) = keymap.get_mut(key) {
            *slot = action;
        }
        self.write(layer, keymap, window, cx);
    }

    fn swap(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        if from == to || !self.editable(from) || !self.editable(to) {
            return;
        }
        let (Some(moved), Some(replaced)) = (self.action(layer, from), self.action(layer, to))
        else {
            return;
        };
        let Some(mut keymap) = self.keymap(layer).cloned() else {
            return;
        };
        if let Some(slot) = keymap.get_mut(to) {
            *slot = moved;
        }
        if let Some(slot) = keymap.get_mut(from) {
            *slot = replaced;
        }
        self.deselect(cx);
        self.write(layer, keymap, window, cx);
    }

    fn write(&mut self, layer: Layer, keymap: Keymap, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let profile = keyboard.active;
        let Some(before) = self.keymap(layer).cloned() else {
            return;
        };
        if before == keymap {
            return;
        }
        if let Err(invalid) = keymap.validate(&keyboard.capabilities) {
            window.push_notification(Notification::error(invalid.to_string()), cx);
            return;
        }
        if let Some(slot) = self.keymaps.get_mut(usize::from(layer.number())) {
            *slot = Some(keymap.clone());
        }
        self.revision += 1;
        let written = keymap.clone();
        let applied = self.store.update(cx, |store, cx| {
            store.change::<SetKeymap>(
                &LayerChange {
                    profile,
                    layer,
                    value: before,
                },
                LayerChange {
                    profile,
                    layer,
                    value: keymap,
                },
                Scope::Profile(profile),
                cx,
            )
        });
        cx.spawn_in(window, async move |this, cx| {
            let answer = applied.await;
            this.update_in(cx, |page, window, cx| match answer {
                Ok(()) => {
                    if layer == Layer::Base {
                        page.store
                            .update(cx, |store, cx| store.set_legends(written, cx));
                    }
                }
                Err(_) => page.reload_layer(layer, window, cx),
            })
            .ok();
        })
        .detach();
        self.sync_picker(cx);
        cx.notify();
    }

    fn add_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layer) = self.next_free() else {
            return;
        };
        if let Some(added) = self.added.get_mut(usize::from(layer.number())) {
            *added = true;
        }
        self.choose_layer(layer, window, cx);
    }

    fn run(
        &mut self,
        command: LayerCommand,
        layer: Layer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            LayerCommand::Copy => self.copy_layer(layer, window, cx),
            LayerCommand::PasteReplace => self.paste_layer(layer, false, window, cx),
            LayerCommand::PasteMerge => self.paste_layer(layer, true, window, cx),
            LayerCommand::Duplicate => self.duplicate_layer(layer, window, cx),
            LayerCommand::Delete => self.delete_layer(layer, window, cx),
        }
    }

    fn available(&self, command: LayerCommand, layer: Layer) -> bool {
        let loaded = self.keymaps.iter().all(Option::is_some);
        loaded
            && match command {
                LayerCommand::Copy => true,
                LayerCommand::PasteReplace | LayerCommand::PasteMerge => self.copied.is_some(),
                LayerCommand::Duplicate => layer != Layer::Base && self.next_free().is_some(),
                LayerCommand::Delete => layer != Layer::Base,
            }
    }

    fn copy_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keymap) = self.keymap(layer).cloned() else {
            return;
        };
        self.copied = Some((layer, keymap));
        window.push_notification(
            Notification::info(format!("{} copied", layer_label(layer))),
            cx,
        );
        cx.notify();
    }

    fn paste_layer(
        &mut self,
        layer: Layer,
        merge: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some((_, copied)), Some(target)) = (&self.copied, self.keymap(layer)) else {
            return;
        };
        let (pasted, dropped) = paste(target, copied, layer, merge);
        if dropped > 0 {
            window.push_notification(Notification::warning(dropped_message(dropped)), cx);
        }
        self.write(layer, pasted, window, cx);
    }

    fn duplicate_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        if !self.available(LayerCommand::Duplicate, layer) {
            return;
        }
        let (Some(target), Some(source)) = (self.next_free(), self.keymap(layer).cloned()) else {
            return;
        };
        let Some(empty) = self.keymap(target).cloned() else {
            return;
        };
        let (copy, dropped) = paste(&empty, &source, target, false);
        if dropped > 0 {
            window.push_notification(Notification::warning(dropped_message(dropped)), cx);
        }
        if let Some(added) = self.added.get_mut(usize::from(target.number())) {
            *added = true;
        }
        self.write(target, copy, window, cx);
        self.choose_layer(target, window, cx);
    }

    fn delete_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        if layer == Layer::Base {
            return;
        }
        let opener = Action::Layer(layer);
        for other in Layer::ALL {
            let Some(keymap) = self.keymap(other) else {
                continue;
            };
            let mut changed = keymap.clone();
            for action in &mut changed {
                if other == layer || *action == opener {
                    *action = Action::None;
                }
            }
            self.write(other, changed, window, cx);
        }
        if let Some(added) = self.added.get_mut(usize::from(layer.number())) {
            *added = false;
        }
        if self.layer == layer {
            self.choose_layer(Layer::Base, window, cx);
        }
        cx.notify();
    }

    fn apply_layout(&mut self, layout: LetterLayout, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let layer = self.layer;
        let Some(current) = self.keymap(layer).cloned() else {
            return;
        };
        let from = letter_layout::detect(&current, |key| place(&keyboard, key))
            .unwrap_or(LetterLayout::Qwerty);
        let converted = letter_layout::convert(&current, from, layout);
        self.write(layer, converted, window, cx);
    }

    fn factory_keymap(&self, factory: Factory) -> Option<Keymap> {
        let defaults = self.defaults.as_ref()?;
        let current = self.keymap(self.layer)?;
        let preset = factory.keymap(defaults);
        Some(
            current
                .iter()
                .enumerate()
                .map(|(key, kept)| {
                    if self.managed(key, self.layer).is_some() || self.locked(key, self.layer) {
                        return *kept;
                    }
                    preset
                        .get(key)
                        .copied()
                        .filter(|action| catalog::refusal(*action, self.layer).is_none())
                        .unwrap_or(Action::None)
                })
                .collect(),
        )
    }

    fn apply_factory(&mut self, factory: Factory, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(keymap) = self.factory_keymap(factory) {
            self.write(self.layer, keymap, window, cx);
        }
    }

    fn factory_tile(&self, factory: Factory, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let keymap = self.factory_keymap(factory)?;
        let selected = self.keymap(self.layer) == Some(&keymap);
        Some(
            preset_tile(
                SharedString::from(format!("factory-{}", factory.name())),
                factory.sample(),
                factory.name(),
                selected,
                cx,
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.apply_factory(factory, window, cx)),
            ),
        )
    }

    fn shortcut(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus.is_focused(window) || !event.keystroke.modifiers.secondary(cx) {
            return;
        }
        let command = match event.keystroke.key.as_str() {
            "c" => LayerCommand::Copy,
            "v" => LayerCommand::PasteReplace,
            "d" => LayerCommand::Duplicate,
            _ => return,
        };
        cx.stop_propagation();
        if self.available(command, self.layer) {
            self.run(command, self.layer, window, cx);
        }
    }

    fn face(&self, keyboard: &Keyboard, key: usize, cx: &App) -> KeyFace {
        let layer = self.layer;
        let theme = cx.theme();
        let caps = theme::keycaps();
        if let Some(kind) = self.managed(key, layer) {
            return KeyFace {
                glyph: Some(kind.glyph(caps.legend)),
                tooltip: Some(MANAGED.into()),
                ..KeyFace::default()
            };
        }
        let action = self
            .action(layer, key)
            .or_else(|| {
                (layer == Layer::Base)
                    .then(|| keyboard.legends.get(key).copied())
                    .flatten()
            })
            .unwrap_or_default();
        if self.locked(key, layer) {
            return KeyFace {
                legend: usage::legend(Action::Layer(layer)).into(),
                dimmed: true,
                tooltip: Some(LOCKED.into()),
                ..KeyFace::default()
            };
        }
        if self.selected == Some(key) {
            return KeyFace {
                callout: Some(BIND_PROMPT.into()),
                ..KeyFace::default()
            };
        }
        match action {
            Action::None if layer == Layer::Base => KeyFace {
                glyph: Some(keycap::alert(theme.warning)),
                tooltip: Some(UNBOUND.into()),
                ..KeyFace::default()
            },
            Action::None => KeyFace::default(),
            action => {
                let profile_name = match action {
                    Action::Function(Function::Profile(slot)) => keyboard
                        .profiles
                        .get(usize::from(slot.get()))
                        .map(|info| info.name.as_str()),
                    _ => None,
                };
                KeyFace {
                    legend: usage::legend(action).into(),
                    glyph: glyph(action, caps.legend, theme.primary),
                    tooltip: catalog::help(action, profile_name).map(SharedString::from),
                    ..KeyFace::default()
                }
            }
        }
    }

    fn keyboard_view(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> KeyboardView {
        let mut selection = Selection::default();
        selection.set(self.selected);
        let caps = theme::keycaps();
        let accent = cx.theme().primary;
        let payloads: Vec<Option<Dragged<Moved>>> = (0..keyboard.key_count())
            .map(|key| {
                let action = self.action(self.layer, key)?;
                (self.editable(key) && action != Action::None).then(|| {
                    let dragged =
                        Dragged::new(Moved(key), SharedString::from(usage::legend(action)));
                    match glyph(action, caps.legend, accent) {
                        Some(glyph) => dragged.glyph(glyph),
                        None => dragged,
                    }
                })
            })
            .collect();
        let clicked = cx.entity().downgrade();
        let cleared = clicked.clone();
        let dropped = clicked.clone();
        let swapped = clicked.clone();
        KeyboardView::new("remap-keyboard", keyboard, |key| {
            self.face(keyboard, key, cx)
        })
        .selection(&selection)
        .fit_width(panel::content_width_beside_side(window))
        .on_key_click(move |key, _, window, cx| {
            clicked
                .update(cx, |page, cx| page.click_key(key, window, cx))
                .ok();
        })
        .on_key_secondary_click(move |key, window, cx| {
            cleared
                .update(cx, |page, cx| page.assign(key, Action::None, window, cx))
                .ok();
        })
        .on_key_drop::<Action>(move |key, action, window, cx| {
            dropped
                .update(cx, |page, cx| {
                    page.deselect(cx);
                    page.assign(key, *action, window, cx);
                })
                .ok();
        })
        .on_key_drag::<Moved>(move |key| payloads.get(key).cloned().flatten())
        .on_key_drop::<Moved>(move |key, moved, window, cx| {
            swapped
                .update(cx, |page, cx| page.swap(moved.0, key, window, cx))
                .ok();
        })
    }

    fn layers(&self, cx: &mut Context<Self>) -> Div {
        let pills: Vec<AnyElement> = Layer::ALL
            .into_iter()
            .filter(|layer| self.shown(*layer))
            .map(|layer| {
                let trailing = h_flex()
                    .gap_1()
                    .items_center()
                    .when(
                        layer != Layer::Base
                            && self.keymaps.iter().all(Option::is_some)
                            && !self.opens(layer),
                        |this| {
                            this.child(layers::warning(
                                SharedString::from(format!("remap-unreachable-{}", layer.number())),
                                UNREACHABLE,
                                cx,
                            ))
                        },
                    )
                    .child(self.layer_menu(layer, cx))
                    .into_any_element();
                layer_pill(
                    format!("remap-layer-{}", layer.number()),
                    layer,
                    self.layer == layer,
                    Some(trailing),
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.choose_layer(layer, window, cx)),
                )
                .into_any_element()
            })
            .collect();
        let full = self.next_free().is_none();
        let add = layers::add_button("remap-add-layer", full, cx)
            .when(!full, |this| {
                this.on_click(cx.listener(|this, _, window, cx| this.add_layer(window, cx)))
            })
            .into_any_element();
        layers_column(pills, Some(add), cx)
    }

    fn layer_menu(&self, layer: Layer, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.entity().downgrade();
        let shortcut = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl+"
        };
        let items: Vec<Item> = LayerCommand::ALL
            .into_iter()
            .map(|command| {
                let item = Item::new(command.label())
                    .disabled(!self.available(command, layer))
                    .danger(command == LayerCommand::Delete);
                match command.shortcut() {
                    Some(key) => item.detail(format!("{shortcut}{key}")),
                    None => item,
                }
            })
            .collect();
        let muted = cx.theme().muted_foreground;
        let hover = cx.theme().secondary_hover;
        div()
            .id(SharedString::from(format!(
                "remap-layer-menu-{}",
                layer.number()
            )))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Dropdown::new(SharedString::from(format!(
                    "remap-layer-dropdown-{}",
                    layer.number()
                )))
                .menu_width(px(MENU_WIDTH))
                .trigger(move |open, _| {
                    div()
                        .size_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .text_color(muted)
                        .when(open, |this| this.bg(hover))
                        .hover(move |style| style.bg(hover))
                        .child(Icon::new(Catalog::EllipsisVertical).small())
                        .into_any_element()
                })
                .items(items)
                .on_select(move |index, window, cx| {
                    let Some(command) = LayerCommand::ALL.get(index).copied() else {
                        return;
                    };
                    page.update(cx, |page, cx| page.run(command, layer, window, cx))
                        .ok();
                }),
            )
            .into_any_element()
    }

    fn presets_card(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Remap keys",
            Some("Drag keys from the right onto the keyboard, or pick a preset".into()),
            cx,
        );
        let Some(keymap) = self.keymap(self.layer) else {
            return card;
        };
        let detected = letter_layout::detect(keymap, |key| place(keyboard, key));
        let mut tiles: Vec<Stateful<Div>> = Vec::new();
        tiles.extend(self.factory_tile(Factory::Default, cx));
        tiles.extend(self.factory_tile(Factory::Function, cx));
        for layout in LetterLayout::ALL
            .into_iter()
            .filter(|layout| *layout != LetterLayout::Qwerty)
        {
            if layout == LetterLayout::Alphabetical {
                tiles.extend(self.factory_tile(Factory::Leet, cx));
            }
            tiles.push(
                preset_tile(
                    SharedString::from(format!("letter-layout-{}", layout.name())),
                    layout.sample(),
                    layout.name(),
                    detected == Some(layout),
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.apply_layout(layout, window, cx)),
                ),
            );
        }
        card.child(panel::field(
            "Select a preset",
            h_flex().gap_2().flex_wrap().children(tiles),
            cx,
        ))
    }
}

impl Render for RemapPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page();
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_advanced(cx);
        }
        self.seen = Instant::now();
        let view = self.keyboard_view(&keyboard, window, cx);
        let cards = h_flex()
            .w_full()
            .items_start()
            .gap_4()
            .child(
                div()
                    .flex_none()
                    .w(px(SIDE_CARD_WIDTH))
                    .child(self.presets_card(&keyboard, cx)),
            )
            .child(
                div().flex_1().min_w_0().child(
                    v_flex()
                        .w_full()
                        .p_6()
                        .rounded(cx.theme().radius_lg)
                        .bg(cx.theme().group_box)
                        .child(self.picker.clone()),
                ),
            );
        let body = v_flex()
            .w_full()
            .gap_5()
            .child(panel::heading(
                Page::Remap.title(),
                Some(REMAP_HELP.into()),
                div(),
                cx,
            ))
            .child(cards);
        panel::page_with_stage(
            "remap-body",
            panel::stage_with_side(self.layers(cx), view),
            body,
        )
        .track_focus(&self.focus)
        .on_key_down(cx.listener(Self::shortcut))
    }
}

fn preset_tile(
    id: SharedString,
    sample: &'static str,
    name: &'static str,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    v_flex()
        .id(id)
        .size(px(LAYOUT_TILE_SIZE))
        .items_center()
        .justify_center()
        .gap_1()
        .rounded(theme.radius)
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.border
        })
        .bg(theme.secondary)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(div().text_sm().font_weight(FontWeight::BOLD).child(sample))
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(name),
        )
}

fn place(keyboard: &Keyboard, key: usize) -> Option<(u16, u16)> {
    if let Some(geometry) = &keyboard.geometry {
        return geometry
            .get(key)
            .copied()
            .flatten()
            .map(|placed| (placed.y, placed.x));
    }
    let columns = keyboard.columns().max(1);
    Some((
        u16::try_from(key / columns).ok()?,
        u16::try_from(key % columns).ok()?,
    ))
}

fn paste(target: &Keymap, copied: &Keymap, layer: Layer, merge: bool) -> (Keymap, usize) {
    let mut pasted = target.clone();
    let mut dropped = 0;
    for (slot, action) in pasted.iter_mut().zip(copied.iter().copied()) {
        let allowed = catalog::refusal(action, layer).is_none();
        if !allowed {
            dropped += 1;
        }
        if merge {
            if allowed && action != Action::None {
                *slot = action;
            }
        } else {
            *slot = if allowed { action } else { Action::None };
        }
    }
    (pasted, dropped)
}

fn dropped_message(dropped: usize) -> String {
    if dropped == 1 {
        "1 action can't go on this layer and was left out".to_owned()
    } else {
        format!("{dropped} actions can't go on this layer and were left out")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keymap(actions: &[Action]) -> Keymap {
        actions.iter().copied().collect()
    }

    #[test]
    fn pasting_replaces_every_key_and_merging_keeps_the_empty_ones() {
        let target = keymap(&[Action::Keyboard(0x04), Action::Keyboard(0x05), Action::None]);
        let copied = keymap(&[Action::None, Action::Keyboard(0x06), Action::Keyboard(0x07)]);
        assert_eq!(
            paste(&target, &copied, Layer::Two, false),
            (copied.clone(), 0)
        );
        assert_eq!(
            paste(&target, &copied, Layer::Two, true),
            (
                keymap(&[
                    Action::Keyboard(0x04),
                    Action::Keyboard(0x06),
                    Action::Keyboard(0x07)
                ]),
                0
            )
        );
    }

    #[test]
    fn pasting_leaves_out_what_the_layer_cannot_hold() {
        let target = keymap(&[Action::Keyboard(0x04), Action::Keyboard(0x05)]);
        let copied = keymap(&[
            Action::Layer(Layer::One),
            Action::Function(Function::LayerLock),
        ]);
        assert_eq!(
            paste(&target, &copied, Layer::One, false),
            (
                keymap(&[Action::None, Action::Function(Function::LayerLock)]),
                1
            )
        );
        assert_eq!(
            paste(&target, &copied, Layer::Base, true),
            (
                keymap(&[Action::Layer(Layer::One), Action::Keyboard(0x05)]),
                1
            )
        );
    }
}
