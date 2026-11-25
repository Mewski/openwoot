use gpui_kit::{
    Context, Div, IntoElement, ParentElement as _, SharedString, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lightbar::{
    Comparator, Comparison, Condition, Entry, Group, Logic, MAX_TERMS, Operand, Signal, Term,
};

use super::{
    LightbarPage,
    entry::{self, COMPARATORS, Spot, TIMINGS, Timing},
    number::Field,
    parameters::Socket,
};
use crate::{
    dropdown::{Dropdown, Item},
    page::lighting::{Choice, add_button, choices, param_label},
    panel,
};

const COMPARATOR_WIDTH: f32 = 72.0;
const TYPES: [(Logic, &str); 3] = [
    (Logic::Single, "Single condition"),
    (Logic::All, "All conditions required"),
    (Logic::Any, "Any condition required"),
];
const ORDINALS: [&str; 2] = ["First condition", "Second condition"];

impl LightbarPage {
    pub(super) fn conditions(&self, entry: &Entry, cx: &Context<Self>) -> Div {
        let Some(condition) = entry.condition.as_ref() else {
            return v_flex();
        };
        let nested = condition.groups.len() > 1;
        let logic = if nested {
            condition.logic
        } else {
            condition
                .groups
                .first()
                .map_or(Logic::Single, |group| group.logic)
        };
        let mut body = v_flex().w_full().gap_5().child(panel::field(
            "Condition type",
            Self::type_menu("condition-type", logic, None, nested, cx),
            cx,
        ));
        for (index, group) in condition.groups.iter().enumerate() {
            body = body.child(self.group_box(index, group, nested, cx));
        }
        let roomy = condition
            .groups
            .iter()
            .rposition(entry::can_add_term)
            .filter(|_| logic != Logic::Single || nested);
        body.child(
            h_flex()
                .w_full()
                .gap_2()
                .when_some(roomy, |this, roomy| {
                    this.child(div().flex_1().child(
                        add_button("add-term", "Add condition", cx).on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit_condition(
                                    |condition| {
                                        if let Some(group) = condition.groups.get_mut(roomy) {
                                            entry::add_term(group);
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            },
                        )),
                    ))
                })
                .when(entry::can_add_group(condition), |this| {
                    this.child(div().flex_1().child(
                        add_button("add-group", "Add group", cx).on_click(cx.listener(
                            |this, _, window, cx| {
                                this.edit_condition(entry::add_group, window, cx);
                            },
                        )),
                    ))
                }),
        )
    }

    fn type_menu(
        id: &'static str,
        current: Logic,
        group: Option<usize>,
        nested: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let scope = group.map_or_else(|| id.to_owned(), |index| format!("{id}-{index}"));
        Dropdown::new(SharedString::from(scope))
            .items(TYPES.iter().map(|(logic, label)| {
                Item::new(*label).disabled(*logic == Logic::Single && nested && group.is_none())
            }))
            .selected(TYPES.iter().position(|(logic, _)| *logic == current))
            .on_select(move |index, window, cx| {
                let Some((logic, _)) = TYPES.get(index).copied() else {
                    return;
                };
                page.update(cx, |page, cx| {
                    page.set_logic(group, logic, nested, window, cx);
                })
                .ok();
            })
    }

    fn group_box(&self, index: usize, group: &Group, nested: bool, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let mut content = v_flex().w_full().gap_4();
        if nested {
            content = content.child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .child(param_label(format!("Group {}", index + 1), cx))
                    .child(div().flex_1().child(Self::type_menu(
                        "group-type",
                        group.logic,
                        Some(index),
                        false,
                        cx,
                    )))
                    .child(remove_link(
                        SharedString::from(format!("remove-group-{index}")),
                        cx.listener(move |this, _, window, cx| {
                            this.edit_entry_condition(
                                |condition| {
                                    let kept = condition
                                        .as_mut()
                                        .is_none_or(|kept| entry::remove_group(kept, index));
                                    if !kept {
                                        *condition = None;
                                    }
                                },
                                window,
                                cx,
                            );
                        }),
                        cx,
                    )),
            );
        }
        for (term, value) in group.terms.iter().enumerate() {
            let spot = Spot { group: index, term };
            content = content.child(self.term_box(spot, value, group.terms.len(), cx));
        }
        if nested {
            v_flex()
                .w_full()
                .p_3()
                .rounded(px(8.0))
                .border_1()
                .border_color(theme.border)
                .child(content)
        } else {
            content
        }
    }

    fn term_box(&self, spot: Spot, term: &Term, terms: usize, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let timing = Timing::of(term);
        let title = ORDINALS.get(spot.term).copied().unwrap_or("Condition");
        let header = h_flex()
            .w_full()
            .justify_between()
            .items_center()
            .child(div().text_base().text_color(theme.foreground).child(title))
            .when(terms > 1 || spot.group > 0, |this| {
                this.child(remove_link(
                    SharedString::from(format!("remove-term-{}-{}", spot.group, spot.term)),
                    cx.listener(move |this, _, window, cx| {
                        this.edit_entry_condition(
                            |condition| {
                                let kept = condition.as_mut().is_none_or(|kept| {
                                    entry::remove_term(kept, spot.group, spot.term)
                                });
                                if !kept {
                                    *condition = None;
                                }
                            },
                            window,
                            cx,
                        );
                    }),
                    cx,
                ))
            });
        let trigger = panel::field(
            "Condition trigger",
            choices(
                SharedString::from(format!("timing-{}-{}", spot.group, spot.term)),
                TIMINGS.map(Timing::label),
                TIMINGS.iter().position(|choice| *choice == timing),
                Choice::Outlined,
                cx.listener(move |this, index: &usize, window, cx| {
                    if let Some(choice) = TIMINGS.get(*index).copied() {
                        this.edit_term(spot, |term| *term = choice.apply(term), window, cx);
                    }
                }),
                cx,
            ),
            cx,
        );
        v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded(px(8.0))
            .bg(theme.background)
            .child(header)
            .child(trigger)
            .child(panel::field("Condition", self.sentence(spot, term, cx), cx))
    }

    fn sentence(&self, spot: Spot, term: &Term, cx: &Context<Self>) -> Div {
        let timing = Timing::of(term);
        let signal = entry::term_signal(term);
        let opener = match timing {
            Timing::While => "While",
            Timing::Matched | Timing::Changed => "When",
        };
        let mut line = h_flex()
            .w_full()
            .flex_wrap()
            .gap_2()
            .items_center()
            .text_sm()
            .child(opener)
            .child(
                div()
                    .w(px(220.0))
                    .child(self.signal_menu(Some(signal), Socket::Term(spot), cx)),
            )
            .when(matches!(signal, Signal::Host(_)), |this| {
                this.child(self.number(Field::SignalHost(spot), cx))
            });
        match term {
            Term::Trigger { .. } => {
                line = line.child("changes,");
            }
            Term::Compare(comparison) => {
                line = line.children(self.comparison(spot, *comparison, cx));
            }
        }
        line = match timing {
            Timing::While => line.child("the effect shows."),
            Timing::Matched | Timing::Changed => line
                .child("the effect shows for")
                .child(self.number(Field::Time(spot), cx)),
        };
        line
    }

    fn comparison(&self, spot: Spot, comparison: Comparison, cx: &Context<Self>) -> Vec<Div> {
        let operand = match comparison.operand {
            Operand::Constant(_) => None,
            Operand::Signal(signal) => Some(signal),
        };
        let mut parts = vec![
            div().w(px(COMPARATOR_WIDTH)).child(Self::comparator_menu(
                spot,
                comparison.comparator,
                cx,
            )),
            div()
                .w(px(150.0))
                .child(self.signal_menu(operand, Socket::Operand(spot), cx)),
        ];
        match operand {
            None => parts.push(self.number(Field::Constant(spot), cx)),
            Some(Signal::Host(_)) => parts.push(self.number(Field::OperandHost(spot), cx)),
            Some(Signal::Builtin(_)) => {}
        }
        parts.push(div().child(","));
        parts
    }

    fn comparator_menu(
        spot: Spot,
        current: Comparator,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        Dropdown::new(SharedString::from(format!(
            "comparator-{}-{}",
            spot.group, spot.term
        )))
        .items(COMPARATORS.iter().map(|comparator| {
            Item::new(entry::comparator_symbol(*comparator)).detail(comparator_name(*comparator))
        }))
        .selected(
            COMPARATORS
                .iter()
                .position(|comparator| *comparator == current),
        )
        .menu_width(px(200.0))
        .on_select(move |index, window, cx| {
            let Some(comparator) = COMPARATORS.get(index).copied() else {
                return;
            };
            page.update(cx, |page, cx| {
                page.edit_term(
                    spot,
                    |term| {
                        if let Term::Compare(comparison) = term {
                            comparison.comparator = comparator;
                        }
                    },
                    window,
                    cx,
                );
            })
            .ok();
        })
    }

    fn set_logic(
        &mut self,
        group: Option<usize>,
        logic: Logic,
        nested: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_condition(
            |condition| match (group, nested) {
                (None, true) => condition.logic = logic,
                (None, false) => {
                    if let Some(first) = condition.groups.get_mut(0) {
                        set_group_logic(first, logic);
                    }
                }
                (Some(index), _) => {
                    if let Some(group) = condition.groups.get_mut(index) {
                        set_group_logic(group, logic);
                    }
                }
            },
            window,
            cx,
        );
    }

    fn edit_condition(
        &mut self,
        change: impl FnOnce(&mut Condition),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_entry_condition(
            |condition| {
                if let Some(condition) = condition {
                    change(condition);
                }
            },
            window,
            cx,
        );
    }

    fn edit_entry_condition(
        &mut self,
        change: impl FnOnce(&mut Option<Condition>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(|entry| change(&mut entry.condition), window, cx);
        self.sync_entry_controls(window, cx);
    }

    fn edit_term(
        &mut self,
        spot: Spot,
        change: impl FnOnce(&mut Term),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_chosen(
            |entry| {
                if let Some(term) = entry::term_mut(entry, spot) {
                    change(term);
                }
            },
            window,
            cx,
        );
        self.sync_entry_controls(window, cx);
    }
}

fn set_group_logic(group: &mut Group, logic: Logic) {
    group.logic = logic;
    match logic {
        Logic::Single => group.terms.truncate(1),
        Logic::All | Logic::Any => {
            while group.terms.len() < MAX_TERMS.min(2) {
                entry::add_term(group);
                group.logic = logic;
            }
        }
    }
}

fn remove_link(
    id: SharedString,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    cx: &Context<LightbarPage>,
) -> impl IntoElement {
    use gpui_kit::{InteractiveElement as _, StatefulInteractiveElement as _};
    let theme = cx.theme();
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded(px(6.0))
        .text_xs()
        .text_color(theme.muted_foreground)
        .cursor_pointer()
        .hover(|style| style.bg(theme.secondary).text_color(theme.foreground))
        .child("Remove")
        .on_click(on_click)
}

const fn comparator_name(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Equal => "equal to",
        Comparator::NotEqual => "not equal to",
        Comparator::Greater => "greater than",
        Comparator::GreaterOrEqual => "at least",
        Comparator::Less => "less than",
        Comparator::LessOrEqual => "at most",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_condition_type_sets_the_number_of_conditions() {
        let mut condition = entry::default_condition();
        let Some(group) = condition.groups.get_mut(0) else {
            panic!("a default condition has one group");
        };
        set_group_logic(group, Logic::All);
        assert_eq!(group.terms.len(), 2);
        assert_eq!(group.logic, Logic::All);
        set_group_logic(group, Logic::Any);
        assert_eq!(group.terms.len(), 2);
        set_group_logic(group, Logic::Single);
        assert_eq!(group.terms.len(), 1);
        assert_eq!(group.logic, Logic::Single);
    }
}
