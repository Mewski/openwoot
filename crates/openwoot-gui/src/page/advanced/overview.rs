use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, MAX_ENTRIES, Resolution},
    keys::{Action, Layer},
};

use super::{AdvancedPage, Kind, kind};
use crate::{
    page::remap::{binding, chip, layer_label},
    panel, theme,
};

const ADD_ORDER: [Kind; 5] = [
    Kind::DepthPriority,
    Kind::Socd,
    Kind::DynamicKeystroke,
    Kind::ModTap,
    Kind::Toggle,
];
const ADD_DESCRIPTION: &str = "Select a type of advanced key from the list below.";
const ACTIVE_TITLE: &str = "Active Advanced Keys";
const NONE_ACTIVE: &str = "No Advanced Keys configured yet.";
const SELECT_TO_EDIT: &str = "Select an Advanced Key to edit";
const MARK_HEIGHT: f32 = 36.0;
const TILE_RADIUS: f32 = 6.0;
const TILE_TEXT_GAP: f32 = 4.0;
const TEXT_LINE: f32 = 19.2;
const COUNT_HEIGHT: f32 = 20.0;
const COUNT_PADDING: f32 = 4.0;
const COUNT_RADIUS: f32 = 6.0;
const COUNT_TEXT: f32 = 12.0;
const ACTIVE_MIN_HEIGHT: f32 = 360.0;
const LIST_PADDING_X: f32 = 16.0;
const LIST_PADDING_Y: f32 = 12.0;
const LIST_GAP: f32 = 16.0;
const LIST_RADIUS: f32 = 6.0;
const GROUP_GAP: f32 = 8.0;
const LABEL_TEXT: f32 = 14.0;
const LABEL_LINE: f32 = 16.8;
const ROW_HEIGHT: f32 = 56.0;
const ROW_PADDING_X: f32 = 8.0;
const ROW_PADDING_Y: f32 = 4.0;
const ROW_GAP: f32 = 8.0;
const ROW_RADIUS: f32 = 6.0;
const KEYS_WIDTH: f32 = 88.0;
const KEYS_GAP: f32 = 4.0;
const DIVIDER_OPACITY: f32 = 0.6;
const SUMMARY_BINDING: f32 = 40.0;
const SUMMARY_TEXT: f32 = 12.8;
const UNAVAILABLE_OPACITY: f32 = 0.5;

impl AdvancedPage {
    pub(super) fn add_card(&self, cx: &mut Context<Self>) -> Div {
        let available = self.entries().len() < MAX_ENTRIES
            && self.entries.is_some()
            && self.keymaps.iter().all(Option::is_some);
        let tiles: Vec<Stateful<Div>> = ADD_ORDER
            .into_iter()
            .map(|kind| Self::kind_tile(kind, available, cx))
            .collect();
        panel::section("Add Advanced Key", Some(ADD_DESCRIPTION.into()), cx)
            .child(v_flex().gap_2().children(tiles))
    }

    fn kind_tile(kind: Kind, available: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (fill, hover, heading, description) = (
            theme.accent,
            theme.secondary,
            theme.foreground,
            theme.group_box_foreground,
        );
        let tile = h_flex()
            .id(SharedString::from(format!("add-{kind:?}")))
            .w_full()
            .gap_3()
            .px_4()
            .py_3()
            .items_center()
            .rounded(px(TILE_RADIUS))
            .bg(fill)
            .child(
                h_flex()
                    .flex_none()
                    .w(px(MARK_HEIGHT))
                    .justify_center()
                    .child(kind.mark(px(MARK_HEIGHT), theme::keycaps().legend)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(TILE_TEXT_GAP))
                    .text_base()
                    .line_height(px(TEXT_LINE))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(heading)
                            .child(kind.title()),
                    )
                    .child(div().text_color(description).child(kind.help())),
            );
        if available {
            tile.cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_creation(kind, window, cx);
                }))
        } else {
            tile.opacity(UNAVAILABLE_OPACITY)
        }
    }

    pub(super) fn active_card(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let count = div()
            .h(px(COUNT_HEIGHT))
            .px(px(COUNT_PADDING))
            .flex()
            .items_center()
            .rounded(px(COUNT_RADIUS))
            .bg(theme.switch)
            .text_size(px(COUNT_TEXT))
            .font_weight(FontWeight::BOLD)
            .child(SharedString::from(format!(
                "{} / {MAX_ENTRIES}",
                self.entries().len()
            )));
        let card = panel::section_with_inline(ACTIVE_TITLE, count, cx).min_h(px(ACTIVE_MIN_HEIGHT));
        if self.entries().is_empty() {
            return card.child(
                div()
                    .text_base()
                    .line_height(px(TEXT_LINE))
                    .text_color(theme.muted_foreground)
                    .child(NONE_ACTIVE),
            );
        }
        let (label, list) = (theme.sidebar_foreground, theme.tab_active);
        let mut groups = v_flex()
            .px(px(LIST_PADDING_X))
            .py(px(LIST_PADDING_Y))
            .gap(px(LIST_GAP))
            .rounded(px(LIST_RADIUS))
            .bg(list)
            .child(
                div()
                    .text_base()
                    .line_height(px(TEXT_LINE))
                    .child(SELECT_TO_EDIT),
            );
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
                    .gap(px(GROUP_GAP))
                    .child(
                        div()
                            .text_size(px(LABEL_TEXT))
                            .line_height(px(LABEL_LINE))
                            .font_weight(FontWeight::BOLD)
                            .text_color(label)
                            .child(SharedString::from(layer_label(layer).to_uppercase())),
                    )
                    .children(rows),
            );
        }
        card.child(groups)
    }

    fn entry_row(&self, entry: advanced::Key, cx: &mut Context<Self>) -> Stateful<Div> {
        let (fill, hover, icon) = {
            let theme = cx.theme();
            (theme.accent, theme.secondary, theme.foreground)
        };
        let kind = Kind::of(&entry.behaviour);
        let id = format!("entry-{}-{}", entry.layer.number(), entry.key.get());
        let keys: Vec<Div> = super::rules::bindings(&entry)
            .map(|(key, _)| {
                let action = self.action_of(Layer::Base, usize::from(key.get()));
                chip(action, self.legend(action, cx), cx)
            })
            .collect();
        h_flex()
            .id(SharedString::from(format!("{id}-row")))
            .w_full()
            .h(px(ROW_HEIGHT))
            .px(px(ROW_PADDING_X))
            .py(px(ROW_PADDING_Y))
            .gap(px(ROW_GAP))
            .items_center()
            .rounded(px(ROW_RADIUS))
            .bg(fill)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(
                h_flex()
                    .flex_none()
                    .w(px(KEYS_WIDTH))
                    .gap(px(KEYS_GAP))
                    .justify_center()
                    .children(keys),
            )
            .child(divider(cx))
            .child(
                h_flex()
                    .flex_none()
                    .w(px(MARK_HEIGHT))
                    .justify_center()
                    .child(kind.mark(px(MARK_HEIGHT), theme::keycaps().legend)),
            )
            .child(h_flex().flex_1().min_w_0().child(self.summary(&entry, cx)))
            .child(divider(cx))
            .child(
                panel::ActionButton::icon_only(
                    SharedString::from(format!("{id}-delete")),
                    Catalog::Trash,
                )
                .text_color(icon)
                .tooltip("Delete")
                .on_click(cx.listener(move |_, _, window, cx| {
                    cx.stop_propagation();
                    Self::confirm_delete(entry, window, cx);
                })),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(entry.key, entry.layer, window, cx);
            }))
    }

    fn summary(&self, entry: &advanced::Key, cx: &Context<Self>) -> Div {
        let bindings = |actions: &[Action]| {
            h_flex().gap(px(ROW_GAP)).children(
                actions.iter().map(|action| {
                    binding(*action, self.legend(*action, cx), px(SUMMARY_BINDING), cx)
                }),
            )
        };
        let text = |label: String| {
            div()
                .text_size(px(SUMMARY_TEXT))
                .line_height(px(TEXT_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::keycaps().legend)
                .child(SharedString::from(label))
        };
        match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => {
                let actions: Vec<Action> = keystroke
                    .strokes
                    .iter()
                    .map(|stroke| stroke.action)
                    .collect();
                bindings(&actions)
            }
            Behaviour::ModTap(mod_tap) => bindings(&[mod_tap.hold, mod_tap.tap]),
            Behaviour::Toggle(action) => bindings(&[action]),
            Behaviour::Socd(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                let label = kind::socd_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| choice.value == pair.resolution)
                    .map_or_else(String::new, |choice| choice.title);
                text(label)
            }
            Behaviour::DepthPriority(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                let label = kind::bottomed_out_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| {
                        choice.value == pair.both_bottomed_out && choice.value != Resolution::Both
                    })
                    .map_or_else(String::new, |choice| choice.title);
                text(label)
            }
        }
    }
}

fn divider(cx: &Context<AdvancedPage>) -> Div {
    div()
        .flex_none()
        .w(px(1.0))
        .h_full()
        .bg(cx.theme().border)
        .opacity(DIVIDER_OPACITY)
}
