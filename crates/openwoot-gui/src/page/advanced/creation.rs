use std::{collections::BTreeSet, time::Duration};

use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Task, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        notification::Notification,
        tooltip::Tooltip,
        v_flex,
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

use super::{AdvancedPage, Draft, Kind, Mode, layer_phrase, preset::PRESETS};
use crate::{
    page::remap::{ActionPicker, chip, keycap_face},
    theme,
};

const SENSE_INTERVAL: Duration = Duration::from_millis(40);
const VISIBLE_WITHIN: Duration = Duration::from_millis(500);
const PRESSED_TRAVEL: u16 = 1600;
const RELEASED_TRAVEL: u16 = 800;
const SLOT_SIZE: f32 = 72.0;
const REMOVE_SIZE: f32 = 18.0;
const PRESETS_HELP: &str = "Ready-made bindings for common uses; they go on the main layer";
const SOCD_WARNING: &str = "Snappy Tappy (SOCD) is prohibited in some games, such as \
                            Counter-Strike 2; using it in competitive play may lead to penalties";

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
        self.start_sensing(cx);
        self.sync_picker(cx);
        self.picker.update(cx, ActionPicker::stop_listening);
        window.refresh();
        cx.notify();
    }

    pub(super) fn draft_key(
        &mut self,
        key: usize,
        sensed: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(problem) = self.choose_draft_key(key, sensed) {
            if !sensed {
                window.push_notification(Notification::warning(problem), cx);
            }
            return;
        }
        cx.notify();
    }

    fn choose_draft_key(&mut self, key: usize, sensed: bool) -> Result<(), String> {
        let layer = self.layer;
        let taken = self.holder(key, layer).is_some();
        let name = self.key_name(key);
        let Mode::Creating(draft) = &mut self.mode else {
            return Ok(());
        };
        if let Some(position) = draft.keys.iter().position(|chosen| *chosen == key) {
            if !sensed {
                draft.keys.remove(position);
            }
            return Ok(());
        }
        if taken {
            return Err(format!(
                "{name} already has an advanced key on {}",
                layer_phrase(layer)
            ));
        }
        if draft.keys.len() < draft.kind.key_count() {
            draft.keys.push(key);
        } else if !sensed {
            draft.keys.pop();
            draft.keys.push(key);
        }
        Ok(())
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

    fn start_sensing(&mut self, cx: &mut Context<Self>) {
        let device = self.store.read(cx).device();
        let poll = cx.spawn(async move |this, cx| {
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
                    .update(cx, |page, cx| page.sensed(&values, cx))
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

    fn sensed(&mut self, values: &sensors::Values, cx: &mut Context<Self>) {
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
            changed |= self.choose_draft_key(key, true).is_ok();
        }
        if changed {
            cx.notify();
        }
    }

    fn creation_header(
        kind: Kind,
        complete: bool,
        border: gpui_kit::Hsla,
        cx: &mut Context<Self>,
    ) -> Div {
        h_flex()
            .w_full()
            .px_6()
            .py_4()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(kind.title()),
            )
            .child(help(
                SharedString::from(format!("creation-help-{kind:?}")),
                kind.help(),
                cx,
            ))
            .child(div().flex_1())
            .child(
                Button::new("creation-cancel")
                    .label("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.overview(cx))),
            )
            .child(
                Button::new("creation-continue")
                    .primary()
                    .label("Continue")
                    .disabled(!complete)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.continue_creation(window, cx);
                    })),
            )
    }

    pub(super) fn creation_card(&self, draft: &Draft, cx: &mut Context<Self>) -> Div {
        let (border, muted, radius, surface) = {
            let theme = cx.theme();
            (
                theme.border,
                theme.muted_foreground,
                theme.radius_lg,
                theme.group_box,
            )
        };
        let kind = draft.kind;
        let needed = kind.key_count();
        let header = Self::creation_header(kind, draft.keys.len() == needed, border, cx);
        let prompt = if needed == 1 {
            format!(
                "Select a key from your keyboard or the preview above to assign {}",
                kind.title()
            )
        } else {
            format!(
                "Select {needed} keys from your keyboard or the preview above to assign {}",
                kind.title()
            )
        };
        let mut slots: Vec<Div> = Vec::with_capacity(needed);
        for position in 0..needed {
            slots.push(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .child(SharedString::from(format!("Key {}", position + 1))),
                    )
                    .child(self.key_slot(draft, position, cx)),
            );
        }
        let left = v_flex()
            .flex_1()
            .min_w_0()
            .p_6()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(prompt)),
            )
            .child(div().text_sm().text_color(muted).child(kind.help()))
            .child(
                h_flex()
                    .w_full()
                    .pt_6()
                    .pb_2()
                    .gap_4()
                    .justify_center()
                    .children(slots),
            )
            .when(kind == Kind::Socd, |this| {
                this.child(warning(SOCD_WARNING, cx))
            });
        let right = v_flex()
            .flex_1()
            .min_w_0()
            .p_6()
            .gap_3()
            .border_l_1()
            .border_color(border)
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Preset bindings"),
                    )
                    .child(help("presets-help".into(), PRESETS_HELP, cx)),
            )
            .children(self.preset_cards(kind, cx));
        v_flex()
            .w_full()
            .rounded(radius)
            .bg(surface)
            .child(header)
            .child(h_flex().w_full().items_stretch().child(left).child(right))
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
            .rounded(px(6.0))
            .text_sm();
        let Some(key) = draft.keys.get(position).copied() else {
            let active = position == draft.keys.len();
            return slot
                .border_1()
                .border_dashed()
                .border_color(if active { theme.primary } else { theme.border })
                .text_color(theme.muted_foreground)
                .child("Assign");
        };
        let action = self.action_of(self.layer, key);
        let danger = theme.danger;
        let foreground = theme.danger_foreground;
        slot.bg(caps.top)
            .border_1()
            .border_color(theme.border)
            .text_color(caps.legend)
            .cursor_pointer()
            .child(keycap_face(
                action,
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
                    .child(Icon::new(Catalog::Close).xsmall()),
            )
            .tooltip(|window, cx| Tooltip::new("Remove this key").build(window, cx))
            .on_click(cx.listener(move |this, _, _, cx| this.remove_draft_key(position, cx)))
    }

    fn preset_cards(&self, kind: Kind, cx: &mut Context<Self>) -> Vec<Stateful<Div>> {
        let main = self.layer == Layer::Base;
        PRESETS
            .iter()
            .enumerate()
            .filter(|(_, preset)| preset.kind == kind)
            .map(|(position, preset)| {
                let plan = self.preset_plan(preset);
                let applied = plan.as_ref().is_some_and(|(added, replaced)| {
                    added.iter().all(|entry| replaced.contains(entry))
                });
                let refusal = if !main {
                    Some("Only available on the main layer; switch layers to apply")
                } else if plan.is_none() {
                    Some("The keys this preset needs aren't on the main layer")
                } else if applied {
                    Some("Already applied")
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
        let hover = theme.secondary_hover;
        let card = v_flex()
            .id(SharedString::from(format!("preset-{position}")))
            .w_full()
            .p_4()
            .gap_2()
            .rounded(theme.radius)
            .bg(theme.secondary)
            .child(
                h_flex()
                    .gap_1()
                    .children(preset.chips().into_iter().map(|action| chip(action, cx))),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(preset.title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(preset.description),
            );
        match refusal {
            Some(reason) => card
                .opacity(0.5)
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

pub(super) fn help(
    id: SharedString,
    text: &'static str,
    cx: &Context<AdvancedPage>,
) -> Stateful<Div> {
    div()
        .id(id)
        .text_color(cx.theme().muted_foreground)
        .child(Icon::new(Catalog::CircleQuestionMark).small())
        .tooltip(move |window, cx| Tooltip::new(text).build(window, cx))
}

pub(super) fn warning(text: &'static str, cx: &Context<AdvancedPage>) -> Div {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .mt_4()
        .p_4()
        .gap_3()
        .items_start()
        .rounded(theme.radius)
        .bg(theme.secondary)
        .child(
            Icon::new(Catalog::TriangleAlert)
                .text_color(theme.warning)
                .flex_none(),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(text),
        )
}
