use gpui_kit::{
    Context, Div, FontWeight, Hsla, InteractiveElement as _, ParentElement as _, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    advanced::{self, Behaviour, MAX_ENTRIES},
    keys::{Action, Layer},
};

use super::{AdvancedPage, Kind, kind};
use crate::{
    page::remap::{blank_chip, chip, layer_label},
    panel, theme,
};

const ADD_ORDER: [Kind; 5] = [
    Kind::DepthPriority,
    Kind::Socd,
    Kind::DynamicKeystroke,
    Kind::ModTap,
    Kind::Toggle,
];
const MARK_HEIGHT: f32 = 44.0;
const ROW_HEIGHT: f32 = 52.0;
const KEYS_WIDTH: f32 = 96.0;
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
        panel::section(
            "Add advanced key",
            Some("Pick a type, then the keys it applies to".into()),
            cx,
        )
        .child(v_flex().gap_2().children(tiles))
    }

    fn kind_tile(kind: Kind, available: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (fill, hover, muted, radius) = (
            theme.secondary,
            theme.secondary_hover,
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
                    .w(px(MARK_HEIGHT))
                    .justify_center()
                    .child(kind.mark(px(MARK_HEIGHT), theme::keycaps().legend)),
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
            .px_1p5()
            .py_0p5()
            .rounded(px(4.0))
            .bg(theme.secondary)
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .child(SharedString::from(format!(
                "{} / {MAX_ENTRIES}",
                self.entries().len()
            )));
        let card = v_flex()
            .w_full()
            .gap_4()
            .p_6()
            .rounded(theme.radius_lg)
            .bg(theme.group_box)
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Active advanced keys"),
                    )
                    .child(count),
            );
        if self.entries().is_empty() {
            return card.child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No advanced keys yet; pick a type to add one"),
            );
        }
        let mut groups = v_flex().gap_4().child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Select an advanced key to edit"),
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
                    .gap_2()
                    .child(panel::label(layer_label(layer), cx))
                    .children(rows),
            );
        }
        card.child(groups)
    }

    fn entry_row(&self, entry: advanced::Key, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (fill, hover, border, muted, radius) = (
            theme.secondary,
            theme.secondary_hover,
            theme.border,
            theme.muted_foreground,
            theme.radius,
        );
        let kind = Kind::of(&entry.behaviour);
        let id = format!("entry-{}-{}", entry.layer.number(), entry.key.get());
        let keys: Vec<Div> = super::rules::bindings(&entry)
            .map(|(key, _)| chip(self.action_of(Layer::Base, usize::from(key.get())), cx))
            .collect();
        h_flex()
            .id(SharedString::from(format!("{id}-row")))
            .w_full()
            .h(px(ROW_HEIGHT))
            .pr_1()
            .items_center()
            .rounded(radius)
            .bg(fill)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(
                h_flex()
                    .flex_none()
                    .w(px(KEYS_WIDTH))
                    .h_full()
                    .gap_1()
                    .justify_center()
                    .border_r_1()
                    .border_color(border)
                    .children(keys),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .child(kind.mark(px(MARK_HEIGHT * 0.7), theme::keycaps().legend))
                    .child(self.summary(&entry, muted, cx)),
            )
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .pl_1()
                    .border_l_1()
                    .border_color(border)
                    .child(
                        Button::new(SharedString::from(format!("{id}-delete")))
                            .ghost()
                            .small()
                            .icon(Catalog::Trash)
                            .tooltip("Delete")
                            .on_click(cx.listener(move |_, _, window, cx| {
                                cx.stop_propagation();
                                Self::confirm_delete(entry, window, cx);
                            })),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit(entry.key, entry.layer, window, cx);
            }))
    }

    fn summary(&self, entry: &advanced::Key, muted: Hsla, cx: &Context<Self>) -> Div {
        let chips = |actions: &[Action]| {
            h_flex().gap_1().children(actions.iter().map(|action| {
                if *action == Action::None {
                    blank_chip(cx)
                } else {
                    chip(*action, cx)
                }
            }))
        };
        match entry.behaviour {
            Behaviour::DynamicKeystroke(keystroke) => {
                let actions: Vec<Action> = keystroke
                    .strokes
                    .iter()
                    .map(|stroke| stroke.action)
                    .collect();
                chips(&actions)
            }
            Behaviour::ModTap(mod_tap) => chips(&[mod_tap.hold, mod_tap.tap]),
            Behaviour::Toggle(action) => chips(&[action]),
            Behaviour::Socd(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                let label = kind::socd_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| choice.value == pair.resolution)
                    .map_or_else(String::new, |choice| choice.title);
                h_flex()
                    .text_sm()
                    .text_color(muted)
                    .child(SharedString::from(label))
            }
            Behaviour::DepthPriority(pair) => {
                let (first, second) = (self.index_name(entry.key), self.index_name(pair.other));
                let label = kind::bottomed_out_resolutions(&first, &second)
                    .into_iter()
                    .find(|choice| choice.value == pair.both_bottomed_out)
                    .map_or_else(String::new, |choice| {
                        format!("Both fully pressed: {}", choice.title)
                    });
                h_flex()
                    .text_sm()
                    .text_color(muted)
                    .when(!label.is_empty(), |this| {
                        this.child(SharedString::from(label))
                    })
            }
        }
    }
}
