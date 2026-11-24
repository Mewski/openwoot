mod catalog;
mod letter_layout;
mod picker;
mod quick_bind;

use std::time::{Duration, Instant};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, FontWeight, InteractiveElement as _,
    IntoElement, Modifiers, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariant, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        notification::Notification,
        tooltip::Tooltip,
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
use super::{Page, advanced::Kind};
use crate::{
    keyboard::{KeyFace, KeyboardView, Selection},
    panel,
    store::{Keyboard, Scope, Store, StoreEvent},
    usage,
};

const STALE_AFTER: Duration = Duration::from_secs(2);
const SIDE_CARD_WIDTH: f32 = 340.0;
const LAYOUT_TILE_SIZE: f32 = 64.0;
const PILL_HEIGHT: f32 = 38.0;

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

impl RemapPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| ActionPicker::new("Disable", window, cx));
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

    fn copy_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keymap) = self.keymap(layer).cloned() else {
            return;
        };
        self.copied = Some((layer, keymap));
        window.push_notification(Notification::info("Layer copied"), cx);
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

    fn confirm_replace(&self, target: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let Some((source, _)) = self.copied else {
            return;
        };
        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let page = page.clone();
            alert
                .title(format!(
                    "Replace {} with {}?",
                    layer_label(target),
                    layer_label(source)
                ))
                .confirm()
                .ok_text("Replace")
                .ok_variant(ButtonVariant::Danger)
                .on_ok(move |_, window, cx| {
                    page.update(cx, |page, cx| page.paste_layer(target, false, window, cx))
                        .ok();
                    true
                })
        });
    }

    fn confirm_clear(&self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
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
                .title(format!("Clear {}?", layer_label(layer)))
                .description("Every key on this layer loses its action.")
                .confirm()
                .ok_text("Clear")
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

    fn face(&self, keyboard: &Keyboard, key: usize) -> KeyFace {
        let layer = self.layer;
        let base = self
            .action(Layer::Base, key)
            .or_else(|| keyboard.legends.get(key).copied())
            .unwrap_or_default();
        let mut face = KeyFace::default();
        match (layer, self.action(layer, key)) {
            (Layer::Base, Some(Action::None)) => face.dimmed = true,
            (_, Some(Action::None) | None) => {
                face.legend = usage::legend(base).into();
                face.dimmed = true;
            }
            (_, Some(action)) => face.legend = usage::legend(action).into(),
        }
        if let Some(entry) = self.advanced_on(key, layer) {
            face.detail = Some(Kind::of(&entry.behaviour).badge().into());
        }
        face
    }

    fn layers(&self, cx: &mut Context<Self>) -> Div {
        let loaded = self.keymaps.iter().all(Option::is_some);
        let pills: Vec<Stateful<Div>> = Layer::ALL
            .into_iter()
            .map(|layer| {
                let menu = self.layer_menu(layer, loaded, cx);
                layer_pill("remap", layer, self.layer == layer, Some(menu), cx)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose_layer(layer, cx)))
            })
            .collect();
        layers_column(pills, cx)
    }

    fn layer_menu(&self, layer: Layer, loaded: bool, cx: &mut Context<Self>) -> AnyElement {
        let page = cx.entity().downgrade();
        let copied = self.copied.is_some();
        let button = Button::new(SharedString::from(format!(
            "remap-layer-menu-{}",
            layer.number()
        )))
        .ghost()
        .xsmall()
        .icon(Catalog::EllipsisVertical)
        .disabled(!loaded)
        .dropdown_menu(move |menu, _, _| {
            let menu = menu
                .item(menu_item(
                    "Copy layer",
                    true,
                    &page,
                    layer,
                    Self::copy_layer,
                ))
                .item(menu_item(
                    "Paste and replace…",
                    copied,
                    &page,
                    layer,
                    |page, layer, window, cx| page.confirm_replace(layer, window, cx),
                ))
                .item(menu_item(
                    "Paste and merge",
                    copied,
                    &page,
                    layer,
                    |page, layer, window, cx| page.paste_layer(layer, true, window, cx),
                ));
            if layer == Layer::Base {
                menu
            } else {
                menu.separator().item(menu_item(
                    "Clear layer…",
                    true,
                    &page,
                    layer,
                    |page, layer, window, cx| page.confirm_clear(layer, window, cx),
                ))
            }
        });
        div().child(button).into_any_element()
    }

    fn notes(&self, cx: &mut Context<Self>) -> Vec<Div> {
        let managed = self
            .selection
            .iter()
            .any(|key| self.is_managed(key, self.layer));
        let unreachable = self.layer != Layer::Base
            && self.keymaps.iter().all(Option::is_some)
            && !self.opens(self.layer);
        [
            (managed, "Keys with an advanced key can't be remapped"),
            (unreachable, "No key opens this layer"),
        ]
        .into_iter()
        .filter(|(shown, _)| *shown)
        .map(|(_, line)| panel::note(line, cx))
        .collect()
    }

    fn layout_field(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Option<Div> {
        let base = self.keymap(Layer::Base)?;
        let detected = letter_layout::detect(base, |key| place(keyboard, key));
        let tiles: Vec<Stateful<Div>> = LetterLayout::ALL
            .into_iter()
            .filter(|layout| detected.is_some() || *layout != LetterLayout::Qwerty)
            .map(|layout| {
                layout_tile(layout, detected == Some(layout), cx).on_click(
                    cx.listener(move |this, _, window, cx| this.apply_layout(layout, window, cx)),
                )
            })
            .collect();
        Some(panel::field(
            "Letter layout",
            h_flex().gap_2().flex_wrap().children(tiles),
            cx,
        ))
    }

    fn remap_card(&self, keyboard: &Keyboard, cx: &mut Context<Self>) -> Div {
        let card = panel::section(
            "Remap keys",
            Some("Changes apply to the active profile".into()),
            cx,
        )
        .children(self.notes(cx));
        if self.layer == Layer::Base {
            card.children(self.layout_field(keyboard, cx))
        } else {
            card
        }
    }

    fn selection_actions(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .gap_2()
            .child(
                Button::new("remap-select-all")
                    .small()
                    .label("Select all keys")
                    .on_click(cx.listener(|this, _, _, cx| this.select_all(cx))),
            )
            .child(
                Button::new("remap-discard-selection")
                    .small()
                    .label("Discard selection")
                    .disabled(self.selection.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.clear_selection(cx))),
            )
    }
}

impl Render for RemapPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = panel::page();
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return page;
        };
        if self.seen.elapsed() > STALE_AFTER {
            self.refresh_advanced(cx);
        }
        self.seen = Instant::now();
        let weak = cx.entity().downgrade();
        let view = KeyboardView::new("remap-keyboard", &keyboard, |key| self.face(&keyboard, key))
            .selection(&self.selection)
            .fit_width(panel::content_width_beside_side(window))
            .on_key_click(move |key, event, _, cx| {
                weak.update(cx, |page, cx| page.click_key(key, event.modifiers(), cx))
                    .ok();
            });
        let caption = self
            .selection
            .is_empty()
            .then(|| caption("Select keys to remap", cx));
        let cards = h_flex()
            .w_full()
            .items_start()
            .gap_4()
            .child(
                div()
                    .flex_none()
                    .w(px(SIDE_CARD_WIDTH))
                    .child(self.remap_card(&keyboard, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(panel::section("Actions", None, cx).child(self.picker.clone())),
            );
        page.child(panel::stage_with_side(self.layers(cx), view))
            .children(caption)
            .child(panel::heading(
                Page::Remap.title(),
                Some("Shift-, Ctrl- or Cmd-click selects several keys".into()),
                self.selection_actions(cx),
                cx,
            ))
            .child(cards)
    }
}

pub(super) const fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Base layer",
        Layer::One => "Fn layer 1",
        Layer::Two => "Fn layer 2",
        Layer::Three => "Fn layer 3",
    }
}

pub(super) fn layers_column(pills: Vec<Stateful<Div>>, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap_2()
        .child(
            h_flex()
                .gap_1p5()
                .items_center()
                .pb_1()
                .child(panel::label("Layers", cx))
                .child(
                    div()
                        .id("layers-help")
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(Catalog::CircleQuestionMark).xsmall())
                        .tooltip(|window, cx| {
                            Tooltip::new("Fn layers work while a key that opens them is held")
                                .build(window, cx)
                        }),
                ),
        )
        .children(pills)
}

pub(super) fn layer_pill(
    page: &'static str,
    layer: Layer,
    selected: bool,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary;
    let padded = trailing.is_none();
    h_flex()
        .id(SharedString::from(format!(
            "{page}-layer-{}",
            layer.number()
        )))
        .w_full()
        .h(px(PILL_HEIGHT))
        .pl_3()
        .map(|this| if padded { this.pr_3() } else { this.pr_1() })
        .gap_2()
        .items_center()
        .justify_between()
        .rounded(theme.radius)
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.group_box
        })
        .bg(theme.group_box)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(layer_label(layer))
        .children(trailing)
}

pub(super) fn caption(text: &'static str, cx: &App) -> Div {
    h_flex()
        .w_full()
        .justify_center()
        .child(panel::label(text, cx))
}

fn layout_tile(layout: LetterLayout, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary;
    v_flex()
        .id(SharedString::from(format!(
            "letter-layout-{}",
            layout.name()
        )))
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
        .bg(theme.accent)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .child(layout.sample()),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(layout.name()),
        )
}

fn menu_item(
    label: &'static str,
    enabled: bool,
    page: &WeakEntity<RemapPage>,
    layer: Layer,
    run: fn(&mut RemapPage, Layer, &mut Window, &mut Context<RemapPage>),
) -> PopupMenuItem {
    let page = page.clone();
    PopupMenuItem::new(label)
        .disabled(!enabled)
        .on_click(move |_, window, cx| {
            page.update(cx, |page, cx| run(page, layer, window, cx))
                .ok();
        })
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
        "1 key has an advanced key and kept its action".to_owned()
    } else {
        format!("{kept} keys have an advanced key and kept their action")
    }
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
