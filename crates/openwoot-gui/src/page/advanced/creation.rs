use std::{collections::BTreeSet, time::Duration};

use gpui_kit::{
    Context, Div, FontWeight, Hsla, InteractiveElement as _, ParentElement as _, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Task, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, WindowExt as _, h_flex, notification::Notification,
        tooltip::Tooltip, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced,
    endpoints::GetSensorValues,
    keys::{Index, Layer},
    sensors::{self, Reading},
};

use super::{AdvancedPage, Draft, Kind, Mode, preset::PRESETS};
use crate::{
    page::remap::{ActionPicker, keycap_face, preset_chip},
    panel, theme,
};

const SENSE_INTERVAL: Duration = Duration::from_millis(40);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const PRESSED_TRAVEL: u16 = 1600;
const RELEASED_TRAVEL: u16 = 800;
pub(super) const CARD_RADIUS: f32 = 8.0;
pub(super) const CARD_MIN_HEIGHT: f32 = 460.0;
const HEADER_PADDING_X: f32 = 24.0;
const HEADER_PADDING_Y: f32 = 16.0;
const HEADER_GAP: f32 = 8.0;
const TITLE_TEXT: f32 = 20.0;
const TITLE_LINE: f32 = 24.0;
const COLUMN_PADDING: f32 = 24.0;
const COLUMN_GAP: f32 = 8.0;
const SEPARATOR: f32 = 0.5;
const LEFT_GAP: f32 = 32.0;
const TEXT_GAP: f32 = 8.0;
const TEXT_LINE: f32 = 19.2;
const SLOTS_GAP: f32 = 16.0;
const SLOT_GAP: f32 = 8.0;
const SLOT_SIZE: f32 = 80.0;
const SLOT_RADIUS: f32 = 8.0;
const SLOT_TEXT: f32 = 16.0;
const REMOVE_SIZE: f32 = 18.0;
const PRESETS_GAP: f32 = 12.0;
const PRESET_PADDING: f32 = 16.0;
const PRESET_GAP: f32 = 12.0;
const PRESET_RADIUS: f32 = 6.0;
const PRESET_TEXT_GAP: f32 = 4.0;
const PRESET_CHIPS_GAP: f32 = 4.0;
const PRESET_DESCRIPTION_LINE: f32 = 22.4;
const REFUSED_OPACITY: f32 = 0.5;
const PRESETS_TITLE: &str = "Preset bindings";
const PRESETS_HELP: &str = "Select a preset to instantly apply common advanced key configurations.";
const ONLY_MAIN_LAYER: &str = "Only available on the Main Layer.";
const MISSING_KEYS: &str = "The keys this preset needs aren't on the Main Layer.";
const OVER_LIMIT: &str = "This preset would exceed your Advanced Keys limit.";
const ALREADY_APPLIED: &str = "This preset is already applied.";
const TAKEN_TITLE: &str = "Cannot use this key";
const TAKEN: &str = "The pressed key already has an Advanced Key on it.";
const REMOVE_KEY: &str = "Remove this key";
const SOCD_WARNING: &str =
    "Use of Snappy Tappy (SOCD) is prohibited in certain games, such as Counter-Strike 2.";

pub(super) struct Sensing {
    pressed: BTreeSet<usize>,
    _poll: Task<()>,
}

impl AdvancedPage {
    pub(super) fn start_creation(
        &mut self,
        kind: Kind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mode = Mode::Creating(Draft {
            kind,
            keys: Vec::new(),
        });
        self.slot = None;
        self.tester = None;
        self.start_sensing(window, cx);
        self.sync_picker(cx);
        self.picker.update(cx, ActionPicker::stop_listening);
        window.refresh();
        cx.notify();
    }

    pub(super) fn draft_key(&mut self, key: usize, cx: &mut Context<Self>) {
        if self.holder(key, self.layer).is_none() && self.choose_draft_key(key, false) {
            cx.notify();
        }
    }

    fn choose_draft_key(&mut self, key: usize, sensed: bool) -> bool {
        let Mode::Creating(draft) = &mut self.mode else {
            return false;
        };
        if let Some(position) = draft.keys.iter().position(|chosen| *chosen == key) {
            if !sensed {
                draft.keys.remove(position);
            }
            return !sensed;
        }
        if draft.keys.len() < draft.kind.key_count() {
            draft.keys.push(key);
        } else if sensed {
            return false;
        } else {
            draft.keys.pop();
            draft.keys.push(key);
        }
        true
    }

    fn remove_draft_key(&mut self, position: usize, cx: &mut Context<Self>) {
        if let Mode::Creating(draft) = &mut self.mode
            && position < draft.keys.len()
        {
            draft.keys.remove(position);
            cx.notify();
        }
    }

    fn continue_creation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Creating(draft) = self.mode.clone() else {
            return;
        };
        let layer = self.layer;
        let keys: Vec<Index> = draft
            .keys
            .iter()
            .filter_map(|key| u8::try_from(*key).ok().map(Index::new))
            .collect();
        let (Some(first), Some(key)) = (keys.first().copied(), draft.keys.first().copied()) else {
            return;
        };
        let Some(behaviour) = draft.kind.behaviour(self.action_of(layer, key), &keys) else {
            return;
        };
        let mut entries = self.entries().to_vec();
        entries.push(advanced::Key {
            key: first,
            layer,
            behaviour,
        });
        if self.write(&entries, window, cx) {
            self.edit(first, layer, window, cx);
        }
    }

    fn start_sensing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let device = self.store.read(cx).device();
        let poll = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(SENSE_INTERVAL).await;
                let Ok(visible) = this.update(cx, |page, _| page.seen.elapsed() < VISIBLE_WITHIN)
                else {
                    return;
                };
                if !visible {
                    continue;
                }
                let Ok(values) = device
                    .configure::<GetSensorValues, sensors::Values>(Reading::Travel)
                    .await
                else {
                    return;
                };
                if this
                    .update_in(cx, |page, window, cx| page.sensed(&values, window, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        self.sensing = Some(Sensing {
            pressed: BTreeSet::new(),
            _poll: poll,
        });
    }

    fn sensed(&mut self, values: &sensors::Values, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sensing) = &mut self.sensing else {
            return;
        };
        let mut fresh = Vec::new();
        for (key, travel) in values.iter().enumerate() {
            let Some(travel) = *travel else {
                continue;
            };
            if travel >= PRESSED_TRAVEL {
                if sensing.pressed.insert(key) {
                    fresh.push(key);
                }
            } else if travel <= RELEASED_TRAVEL {
                sensing.pressed.remove(&key);
            }
        }
        let mut changed = false;
        for key in fresh {
            if self.holder(key, self.layer).is_some() {
                window.push_notification(Notification::warning(TAKEN).title(TAKEN_TITLE), cx);
            } else {
                changed |= self.choose_draft_key(key, true);
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn card_header(kind: Kind, actions: Div, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let title = SharedString::from(kind.title());
        h_flex()
            .w_full()
            .px(px(HEADER_PADDING_X))
            .py(px(HEADER_PADDING_Y))
            .gap(px(HEADER_GAP))
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .min_w_0()
                    .gap(px(HEADER_GAP))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(TITLE_TEXT))
                            .line_height(px(TITLE_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.foreground)
                            .child(title.clone()),
                    )
                    .child(panel::help(&title, kind.help().into(), cx)),
            )
            .child(actions.flex_none().gap(px(HEADER_GAP)))
    }

    pub(super) fn creation_card(&self, draft: &Draft, cx: &mut Context<Self>) -> Div {
        let kind = draft.kind;
        let needed = kind.key_count();
        let complete = draft.keys.len() == needed;
        let actions = h_flex()
            .child(
                panel::ActionButton::new("creation-cancel", "Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.overview(cx))),
            )
            .child(
                panel::ActionButton::new("creation-continue", "Continue")
                    .primary()
                    .disabled(!complete)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.continue_creation(window, cx);
                    })),
            );
        let header = Self::card_header(kind, actions, cx);
        let theme = cx.theme();
        let (surface, separator, text) =
            (theme.group_box, theme.border, theme.group_box_foreground);
        let prompt = if needed == 1 {
            format!(
                "Select a key from your keyboard or the preview above to assign to {}",
                kind.title()
            )
        } else {
            format!(
                "Select {needed} keys from your keyboard or the preview above to assign {}",
                kind.title()
            )
        };
        let slots: Vec<Div> = (0..needed)
            .map(|position| {
                v_flex()
                    .gap(px(SLOT_GAP))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(SLOT_TEXT))
                            .line_height(px(TEXT_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text)
                            .child(SharedString::from(format!("Key {}", position + 1))),
                    )
                    .child(self.key_slot(draft, position, cx))
            })
            .collect();
        let left = v_flex()
            .flex_1()
            .min_w_0()
            .p(px(COLUMN_PADDING))
            .gap(px(LEFT_GAP))
            .child(
                v_flex()
                    .gap(px(TEXT_GAP))
                    .child(
                        div()
                            .text_base()
                            .line_height(px(TEXT_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text)
                            .child(SharedString::from(prompt)),
                    )
                    .child(panel::description(kind.help(), cx)),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap(px(SLOTS_GAP))
                    .justify_center()
                    .children(slots),
            )
            .when(kind == Kind::Socd, |this| {
                this.child(div().flex_1())
                    .child(panel::warning(SOCD_WARNING, cx))
            });
        let right = self.presets_column(kind, text, cx);
        v_flex()
            .w_full()
            .min_h(px(CARD_MIN_HEIGHT))
            .rounded(px(CARD_RADIUS))
            .bg(surface)
            .child(header)
            .child(
                h_flex()
                    .w_full()
                    .flex_1()
                    .items_stretch()
                    .gap(px(COLUMN_GAP))
                    .child(left)
                    .child(div().flex_none().w(px(SEPARATOR)).bg(separator))
                    .child(right),
            )
    }

    fn presets_column(&self, kind: Kind, text: Hsla, cx: &mut Context<Self>) -> Div {
        v_flex()
            .flex_1()
            .min_w_0()
            .p(px(COLUMN_PADDING))
            .gap(px(TEXT_GAP))
            .child(
                h_flex()
                    .gap(px(TEXT_GAP))
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .line_height(px(TEXT_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text)
                            .child(PRESETS_TITLE),
                    )
                    .child(panel::help(
                        &SharedString::from(PRESETS_TITLE),
                        PRESETS_HELP.into(),
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .gap(px(PRESETS_GAP))
                    .children(self.preset_cards(kind, cx)),
            )
    }

    fn key_slot(&self, draft: &Draft, position: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let caps = theme::keycaps();
        let group = SharedString::from(format!("draft-slot-{position}"));
        let slot = div()
            .id(group.clone())
            .group(group.clone())
            .relative()
            .size(px(SLOT_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(SLOT_RADIUS))
            .text_size(px(SLOT_TEXT))
            .line_height(px(TEXT_LINE));
        let Some(key) = draft.keys.get(position).copied() else {
            let active = position == draft.keys.len();
            return slot
                .border_1()
                .border_dashed()
                .border_color(if active { theme.primary } else { theme.border })
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.sidebar_foreground)
                .child("Assign");
        };
        let action = self.action_of(self.layer, key);
        let legend = self.legend(action, cx);
        let (danger, foreground) = (theme.danger, theme.danger_foreground);
        slot.bg(theme::palette().raised)
            .text_color(caps.legend)
            .cursor_pointer()
            .child(keycap_face(
                action,
                legend,
                px(SLOT_SIZE),
                caps.legend,
                theme.primary,
            ))
            .child(
                div()
                    .absolute()
                    .top(-px(REMOVE_SIZE / 3.0))
                    .right(-px(REMOVE_SIZE / 3.0))
                    .size(px(REMOVE_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(danger)
                    .text_color(foreground)
                    .invisible()
                    .group_hover(group, gpui_kit::Styled::visible)
                    .child(Icon::new(Catalog::Close).size(px(REMOVE_SIZE * 0.7))),
            )
            .tooltip(|window, cx| Tooltip::new(REMOVE_KEY).build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| this.remove_draft_key(position, cx)))
    }

    fn preset_cards(&self, kind: Kind, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let main = self.layer == Layer::Base;
        let base = self.keymaps.first().and_then(Option::as_ref);
        PRESETS
            .iter()
            .enumerate()
            .filter(|(_, preset)| {
                preset.kind == kind && base.is_none_or(|base| preset.offered(base))
            })
            .map(|(position, preset)| {
                let plan = self.preset_plan(preset);
                let applied = plan.as_ref().is_some_and(|(added, replaced)| {
                    added.iter().all(|entry| replaced.contains(entry))
                });
                let present = base.is_some_and(|base| preset.entries(base).is_ok());
                let refusal = if !main {
                    Some(ONLY_MAIN_LAYER)
                } else if !present {
                    Some(MISSING_KEYS)
                } else if plan.is_none() {
                    Some(OVER_LIMIT)
                } else if applied {
                    Some(ALREADY_APPLIED)
                } else {
                    None
                };
                Self::preset_card(position, preset, refusal, cx)
            })
            .collect()
    }

    fn preset_card(
        position: usize,
        preset: &'static super::Preset,
        refusal: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let (fill, hover, text) = (theme.accent, theme.secondary, theme.group_box_foreground);
        let card = v_flex()
            .id(SharedString::from(format!("preset-{position}")))
            .w_full()
            .p(px(PRESET_PADDING))
            .gap(px(PRESET_GAP))
            .rounded(px(PRESET_RADIUS))
            .bg(fill)
            .child(
                h_flex().gap(px(PRESET_CHIPS_GAP)).children(
                    preset
                        .chips()
                        .into_iter()
                        .map(|action| preset_chip(action, cx)),
                ),
            )
            .child(
                v_flex()
                    .gap(px(PRESET_TEXT_GAP))
                    .text_base()
                    .text_color(text)
                    .child(
                        div()
                            .line_height(px(TEXT_LINE))
                            .font_weight(FontWeight::BOLD)
                            .child(preset.title),
                    )
                    .child(
                        div()
                            .line_height(px(PRESET_DESCRIPTION_LINE))
                            .child(preset.description),
                    ),
            );
        match refusal {
            Some(reason) => card
                .opacity(REFUSED_OPACITY)
                .tooltip(move |window, cx| Tooltip::new(reason).build(window, cx)),
            None => card
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_preset(preset, window, cx);
                })),
        }
    }
}
