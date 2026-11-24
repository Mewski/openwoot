mod catalog;
mod letter_layout;
mod picker;
mod quick_bind;

use std::time::{Duration, Instant};

use gpui_kit::{
    AppContext as _, Context, Div, Entity, FontWeight, Hsla, IntoElement, Modifiers,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
        WindowExt as _,
        button::{Button, ButtonVariant},
        h_flex,
        notification::Notification,
        tab::{Tab, TabBar},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour},
    configuration::Validate as _,
    endpoints::{GetAdvancedKeys, GetKeymap, SetKeymap},
    keys::{Action, Keymap, LAYERS, Layer},
    profile::{LayerChange, LayerRef},
};

use self::letter_layout::LetterLayout;
pub(super) use self::{
    catalog::refusal,
    picker::{ActionPicker, PickerEvent, Target},
    quick_bind::usage_of_key,
};
use super::advanced::Kind;
use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const KEYBOARD_MARGIN: f32 = 340.0;
const STALE_AFTER: Duration = Duration::from_secs(2);
const MANAGED: &str =
    "This key has an advanced key configured. To remap it, remove the advanced key first.";

pub(crate) struct RemapPage {
    store: Entity<Store>,
    layer: Layer,
    keymaps: [Option<Keymap>; LAYERS],
    advanced: Option<advanced::Keys>,
    selection: Selection,
    picker: Entity<ActionPicker>,
    copied: Option<(Layer, Keymap)>,
    revision: u64,
    seen: Instant,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy)]
struct Look {
    bound: Hsla,
    managed: Hsla,
}

impl RemapPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| {
            ActionPicker::new(
                "Select keys on the keyboard to bind an action to them",
                "Disable key",
                window,
                cx,
            )
        });
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::ProfileChanged | StoreEvent::Reloaded => {
                    this.load(window, cx);
                }
                StoreEvent::Disconnected => {
                    this.keymaps = [const { None }; LAYERS];
                    this.advanced = None;
                    this.selection.clear();
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
                    this.assign(*action, window, cx);
                },
            ),
        ];
        let mut page = Self {
            store,
            layer: Layer::Base,
            keymaps: [const { None }; LAYERS],
            advanced: None,
            selection: Selection::default(),
            picker,
            copied: None,
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
        let revision = self.revision;
        cx.spawn_in(window, async move |this, cx| {
            let mut loaded = [const { None }; LAYERS];
            for (slot, keymap) in loaded.iter_mut().zip(keymaps) {
                *slot = keymap.await.ok();
            }
            let advanced = advanced.await.ok();
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    page.load(window, cx);
                    return;
                }
                page.keymaps = loaded;
                page.advanced = advanced;
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
                    page.sync_picker(cx);
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
                page.sync_picker(cx);
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

    fn advanced_on(&self, key: usize, layer: Layer) -> Option<&advanced::Key> {
        self.advanced.iter().flatten().find(|entry| {
            entry.layer == layer
                && (usize::from(entry.key.get()) == key
                    || match entry.behaviour {
                        Behaviour::DepthPriority(pair) => usize::from(pair.other.get()) == key,
                        Behaviour::Socd(pair) => usize::from(pair.other.get()) == key,
                        Behaviour::DynamicKeystroke(_)
                        | Behaviour::ModTap(_)
                        | Behaviour::Toggle(_) => false,
                    })
        })
    }

    fn is_managed(&self, key: usize, layer: Layer) -> bool {
        self.advanced_on(key, layer)
            .is_some_and(|entry| Kind::of(&entry.behaviour).owns_actions())
    }

    fn common_action(&self) -> Option<Action> {
        let mut actions = self
            .selection
            .iter()
            .map(|key| self.action(self.layer, key));
        let first = actions.next()??;
        actions.all(|action| action == Some(first)).then_some(first)
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let target =
            (!self.selection.is_empty() && self.keymap(self.layer).is_some()).then(|| Target {
                layer: self.layer,
                current: self.common_action(),
            });
        self.picker
            .update(cx, |picker, cx| picker.set_target(target, cx));
    }

    fn choose_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        self.layer = layer;
        self.sync_picker(cx);
        cx.notify();
    }

    fn click_key(&mut self, key: usize, modifiers: Modifiers, cx: &mut Context<Self>) {
        self.selection.click(key, modifiers);
        self.sync_picker(cx);
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        if let Some(keyboard) = self.store.read(cx).keyboard().cloned() {
            self.selection.select_all(&keyboard);
        }
        self.sync_picker(cx);
        cx.notify();
    }

    fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.selection.clear();
        self.sync_picker(cx);
        cx.notify();
    }

    fn assign(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        let Some(keymap) = self.keymap(layer).cloned() else {
            return;
        };
        if let Some(reason) = refusal(action, layer) {
            window.push_notification(Notification::warning(reason), cx);
            return;
        }
        let mut changed = keymap;
        let mut kept = 0_usize;
        for key in self.selection.iter() {
            if self.is_managed(key, layer) {
                kept += 1;
            } else if let Some(slot) = changed.get_mut(key) {
                *slot = action;
            }
        }
        if kept > 0 {
            window.push_notification(Notification::warning(kept_message(kept)), cx);
        }
        self.write(layer, changed, window, cx);
    }

    fn write(&mut self, layer: Layer, keymap: Keymap, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard() else {
            return;
        };
        let profile = keyboard.active;
        if self.keymap(layer) == Some(&keymap) {
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
            store.apply::<SetKeymap>(
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

    fn copy_layer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keymap) = self.keymap(self.layer).cloned() else {
            return;
        };
        self.copied = Some((self.layer, keymap));
        window.push_notification(
            Notification::info(format!("Copied {}", usage::layer_name(self.layer))),
            cx,
        );
        cx.notify();
    }

    fn paste_layer(&mut self, merge: bool, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        let (Some((_, copied)), Some(target)) = (&self.copied, self.keymap(layer)) else {
            return;
        };
        let (pasted, dropped) = paste(target, copied, layer, merge);
        if dropped > 0 {
            window.push_notification(Notification::warning(dropped_message(dropped)), cx);
        }
        self.write(layer, pasted, window, cx);
    }

    fn confirm_replace(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((source, _)) = self.copied else {
            return;
        };
        let target = self.layer;
        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(format!(
                    "Replace {} with the copied {}?",
                    usage::layer_name(target),
                    usage::layer_name(source)
                ))
                .description(
                    "Every key on this layer takes the action it has on the copied layer. Keys \
                     that cannot go on this layer are left without an action.",
                )
                .confirm()
                .ok_text("Paste and replace")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.paste_layer(false, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn confirm_clear(&self, window: &mut Window, cx: &mut Context<Self>) {
        let layer = self.layer;
        let Some(keymap) = self.keymap(layer) else {
            return;
        };
        let mut cleared = keymap.clone();
        cleared.fill(Action::None);
        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            let cleared = cleared.clone();
            alert
                .title(format!(
                    "Remove every binding from {}?",
                    usage::layer_name(layer)
                ))
                .description("No key does anything while this layer is active.")
                .confirm()
                .ok_text("Clear layer")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    let cleared = cleared.clone();
                    page.update(cx, |page, cx| page.write(layer, cleared, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn apply_layout(&mut self, layout: LetterLayout, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let Some(base) = self.keymap(Layer::Base).cloned() else {
            return;
        };
        let from = letter_layout::detect(&base, |key| place(&keyboard, key))
            .unwrap_or(LetterLayout::Qwerty);
        let converted = letter_layout::convert(&base, from, layout);
        self.write(Layer::Base, converted, window, cx);
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

    fn face(&self, keyboard: &Keyboard, key: usize, look: Look) -> KeyFace {
        let layer = self.layer;
        let base = self
            .action(Layer::Base, key)
            .or_else(|| keyboard.legends.get(key).copied())
            .unwrap_or_default();
        let mut face = KeyFace::default();
        match (layer, self.action(layer, key)) {
            (Layer::Base, Some(Action::None)) => {
                face.detail = Some("off".into());
                face.dimmed = true;
            }
            (Layer::Base, Some(action)) => face.legend = usage::legend(action).into(),
            (_, Some(Action::None) | None) => {
                face.legend = usage::legend(base).into();
                face.dimmed = true;
            }
            (_, Some(action)) => {
                face.legend = usage::legend(action).into();
                face.fill = Some(look.bound);
            }
        }
        if let Some(entry) = self.advanced_on(key, layer) {
            let kind = Kind::of(&entry.behaviour);
            face.detail = Some(kind.badge().into());
            if kind.owns_actions() {
                face.fill = Some(look.managed);
                face.dimmed = false;
            }
        }
        face
    }

    fn layer_bar(&self, cx: &mut Context<Self>) -> Div {
        let loaded = self.keymap(self.layer).is_some();
        let copied = self.copied.as_ref().map(|(layer, _)| *layer);
        h_flex()
            .w_full()
            .gap_2()
            .flex_wrap()
            .items_center()
            .child(
                TabBar::new("remap-layers")
                    .segmented()
                    .selected_index(usize::from(self.layer.number()))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(layer) = Layer::ALL.get(*index) {
                            this.choose_layer(*layer, cx);
                        }
                    }))
                    .children(Layer::ALL.map(|layer| Tab::new().label(usage::layer_name(layer)))),
            )
            .child(div().flex_1())
            .child(
                Button::new("copy-layer")
                    .small()
                    .outline()
                    .icon(IconName::Copy)
                    .label("Copy layer")
                    .disabled(!loaded)
                    .on_click(cx.listener(|this, _, window, cx| this.copy_layer(window, cx))),
            )
            .child(
                Button::new("paste-layer")
                    .small()
                    .outline()
                    .icon(Catalog::Clipboard)
                    .label("Paste and replace")
                    .when_some(copied, |button, source| {
                        button.tooltip(format!(
                            "Replace this layer with the copied {}",
                            usage::layer_name(source)
                        ))
                    })
                    .disabled(!loaded || copied.is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.confirm_replace(window, cx))),
            )
            .child(
                Button::new("merge-layer")
                    .small()
                    .outline()
                    .label("Paste and merge")
                    .tooltip(
                        "Add the bindings of the copied layer, keeping the keys it leaves empty",
                    )
                    .disabled(!loaded || copied.is_none())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.paste_layer(true, window, cx)),
                    ),
            )
            .when(self.layer != Layer::Base, |this| {
                this.child(
                    Button::new("clear-layer")
                        .small()
                        .outline()
                        .icon(Catalog::Eraser)
                        .label("Clear layer")
                        .disabled(!loaded)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_clear(window, cx)),
                        ),
                )
            })
    }

    fn keyboard_section(
        &self,
        keyboard: &Keyboard,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let look = Look {
            bound: theme.primary.opacity(0.22),
            managed: theme.warning.opacity(0.35),
        };
        let page = cx.entity().downgrade();
        let board = KeyboardView::new("remap-keyboard", keyboard, |key| {
            self.face(keyboard, key, look)
        })
        .selection(&self.selection)
        .fit_width(window.viewport_size().width - px(KEYBOARD_MARGIN))
        .on_key_click(move |key, event, _, cx| {
            page.update(cx, |page, cx| page.click_key(key, event.modifiers(), cx))
                .ok();
        });
        let description: SharedString = if self.layer == Layer::Base {
            "What every key does without a Fn key held. Select keys, then pick an action below. \
             Shift-, Ctrl- or Cmd-click adds keys to the selection."
                .into()
        } else {
            format!(
                "What every key does while {} is held. Greyed keys do nothing on this layer.",
                usage::layer_name(self.layer)
            )
            .into()
        };
        let mut section = panel::section(usage::layer_name(self.layer), Some(description), cx)
            .child(div().w_full().overflow_hidden().child(board))
            .child(self.selection_bar(cx));
        if let Some(details) = self.selected_details(cx) {
            section = section.child(details);
        }
        if self.layer != Layer::Base
            && self.keymaps.iter().all(Option::is_some)
            && !self.opens(self.layer)
        {
            section = section.child(panel::note(
                format!(
                    "No key opens {} yet. Bind its Fn key (category Fn layers) to a key on another \
                     layer, or use a Toggle or Mod Tap advanced key.",
                    usage::layer_name(self.layer)
                ),
                cx,
            ));
        }
        section
    }

    fn selection_bar(&self, cx: &mut Context<Self>) -> Div {
        let selected = self.selection.len();
        h_flex()
            .gap_2()
            .flex_wrap()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(match selected {
                        0 => "No key selected".to_owned(),
                        1 => "1 key selected".to_owned(),
                        count => format!("{count} keys selected"),
                    })),
            )
            .child(div().flex_1())
            .child(
                Button::new("remap-select-all")
                    .small()
                    .outline()
                    .label("Select all")
                    .on_click(cx.listener(|this, _, _, cx| this.select_all(cx))),
            )
            .child(
                Button::new("remap-clear-selection")
                    .small()
                    .outline()
                    .label("Clear selection")
                    .disabled(selected == 0)
                    .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
            )
    }

    fn selected_details(&self, cx: &mut Context<Self>) -> Option<Div> {
        let key = self
            .selection
            .first()
            .filter(|_| self.selection.len() == 1)?;
        let theme = cx.theme();
        let chips = Layer::ALL.map(|layer| {
            let action = self.action(layer, key).unwrap_or_default();
            let advanced = self
                .advanced_on(key, layer)
                .map_or_else(String::new, |entry| {
                    format!(" · {}", Kind::of(&entry.behaviour).title())
                });
            div()
                .px_2()
                .py_0p5()
                .rounded(theme.radius)
                .border_1()
                .border_color(if layer == self.layer {
                    theme.primary
                } else {
                    theme.border
                })
                .text_xs()
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(usage::layer_name(layer)),
                        )
                        .child(SharedString::from(format!(
                            "{}{advanced}",
                            usage::name(action)
                        ))),
                )
        });
        Some(
            v_flex()
                .gap_2()
                .child(h_flex().gap_1().flex_wrap().children(chips))
                .when(self.is_managed(key, self.layer), |this| {
                    this.child(div().text_sm().text_color(theme.warning).child(MANAGED))
                }),
        )
    }

    fn presets_section(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let base = self.keymap(Layer::Base);
        let detected =
            base.and_then(|keymap| letter_layout::detect(keymap, |key| place(keyboard, key)));
        let status: SharedString = match (base, detected) {
            (None, _) => "Reading the base layer…".into(),
            (Some(_), Some(layout)) => {
                format!("The letter keys are laid out as {}.", layout.name()).into()
            }
            (Some(_), None) => "The letter keys follow none of these layouts; a preset rearranges \
                                them as if they were QWERTY."
                .into(),
        };
        let buttons = LetterLayout::ALL.map(|layout| {
            let current = detected == Some(layout);
            let unknown_qwerty = detected.is_none() && layout == LetterLayout::Qwerty;
            Button::new(SharedString::from(format!(
                "letter-layout-{}",
                layout.name()
            )))
            .small()
            .outline()
            .label(layout.name())
            .selected(current)
            .disabled(base.is_none() || current || unknown_qwerty)
            .on_click(cx.listener(move |this, _, window, cx| this.apply_layout(layout, window, cx)))
        });
        panel::section(
            "Presets",
            Some(
                "Rearrange the letter keys of the base layer. Keep the computer set to its usual \
                 keyboard layout (for example US): the keyboard sends the rearranged letters \
                 itself."
                    .into(),
            ),
            cx,
        )
        .child(panel::note(status, cx))
        .child(h_flex().gap_2().flex_wrap().children(buttons))
    }
}

impl Render for RemapPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page(
            "Remap",
            "Choose what every key does on the base layer and on the three Fn layers.",
            cx,
        );
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_advanced(cx);
        }
        self.seen = Instant::now();
        let actions = panel::section(
            "Actions",
            Some(
                "Pick an action for the selected keys, search for one, or use Quick bind and \
                 press a key."
                    .into(),
            ),
            cx,
        )
        .child(self.picker.clone());
        page.child(self.layer_bar(cx))
            .child(self.keyboard_section(&keyboard, window, cx))
            .child(actions)
            .when(self.layer == Layer::Base, |page| {
                page.child(self.presets_section(&keyboard, cx))
            })
    }
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
        let allowed = refusal(action, layer).is_none();
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

fn kept_message(kept: usize) -> String {
    if kept == 1 {
        "1 key has an advanced key on this layer and kept its action. Remove the advanced key \
         first to remap it."
            .to_owned()
    } else {
        format!(
            "{kept} keys have an advanced key on this layer and kept their actions. Remove the \
             advanced keys first to remap them."
        )
    }
}

fn dropped_message(dropped: usize) -> String {
    if dropped == 1 {
        "1 binding cannot go on this layer and was left out".to_owned()
    } else {
        format!("{dropped} bindings cannot go on this layer and were left out")
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Function;

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
