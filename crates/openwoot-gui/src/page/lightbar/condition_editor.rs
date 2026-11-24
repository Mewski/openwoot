use gpui_kit::{
    Context, Div, FontWeight, IntoElement, ParentElement as _, SharedString, Styled as _, Window,
    component::{
        ActiveTheme as _, IconName, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        switch::Switch,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};
use openwoot_protocol::lightbar::{
    Builtin, Comparator, Comparison, Condition, Entry, Group, Logic, Operand, Signal, Term,
};

use super::{
    LightbarPage,
    entry::{self, COMPARATORS, LOGICS, Spot, TIMINGS, Timing},
    entry_editor::Socket,
    number::Field,
};
use crate::panel;

impl LightbarPage {
    pub(super) fn condition_rows(&self, entry: &Entry, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let condition = entry.condition.as_ref();
        v_flex()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Conditions"),
                    )
                    .child(div().text_sm().text_color(theme.muted_foreground).child(
                        "Set up the conditions that decide when the effect shows: while a \
                                 data point matches, for a while after it starts to match, or for \
                                 a while after it changes. Without a condition the effect always \
                                 shows.",
                    )),
            )
            .child(panel::row(
                "Show only under a condition",
                None,
                Switch::new("condition-enabled")
                    .checked(condition.is_some())
                    .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                        let enabled = *enabled;
                        this.edit_condition(
                            |condition| *condition = enabled.then(entry::default_condition),
                            window,
                            cx,
                        );
                    })),
                cx,
            ))
            .when_some(condition, |this, condition| {
                this.child(self.condition_body(condition, cx))
            })
    }

    fn condition_body(&self, condition: &Condition, cx: &Context<Self>) -> Div {
        v_flex()
            .gap_2()
            .child(logic_row(
                "Condition type",
                "How the groups combine.",
                condition.logic,
                None,
                cx,
            ))
            .children(
                condition
                    .groups
                    .iter()
                    .enumerate()
                    .map(|(index, group)| self.group_box(index, group, cx)),
            )
            .when(entry::can_add_group(condition), |this| {
                this.child(
                    h_flex().child(
                        Button::new("add-group")
                            .icon(IconName::Plus)
                            .label("Add group")
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_condition(
                                    |condition| {
                                        if let Some(condition) = condition {
                                            entry::add_group(condition);
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            })),
                    ),
                )
            })
    }

    fn group_box(&self, index: usize, group: &Group, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        v_flex()
            .gap_2()
            .p_3()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("Group {}", index + 1)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("remove-group-{index}")))
                            .icon(IconName::Delete)
                            .xsmall()
                            .ghost()
                            .tooltip("Remove the group")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_condition(
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
                            })),
                    ),
            )
            .child(logic_row(
                "Group type",
                "How the conditions of the group combine.",
                group.logic,
                Some(index),
                cx,
            ))
            .children(
                group
                    .terms
                    .iter()
                    .enumerate()
                    .map(|(term, value)| self.term_box(Spot { group: index, term }, value, cx)),
            )
            .when(entry::can_add_term(group), |this| {
                this.child(
                    h_flex().child(
                        Button::new(SharedString::from(format!("add-term-{index}")))
                            .icon(IconName::Plus)
                            .label("Add condition")
                            .xsmall()
                            .outline()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_condition(
                                    |condition| {
                                        if let Some(group) = condition
                                            .as_mut()
                                            .and_then(|condition| condition.groups.get_mut(index))
                                        {
                                            entry::add_term(group);
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            })),
                    ),
                )
            })
    }

    fn term_box(&self, spot: Spot, term: &Term, cx: &Context<Self>) -> Div {
        let timing = Timing::of(term);
        let ordinal = if spot.term == 0 {
            "First Condition"
        } else {
            "Second Condition"
        };
        v_flex()
            .gap_1()
            .pt_1()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(ordinal))
                    .child(
                        Button::new(SharedString::from(format!(
                            "remove-term-{}-{}",
                            spot.group, spot.term
                        )))
                        .icon(IconName::Close)
                        .xsmall()
                        .ghost()
                        .tooltip("Remove the condition")
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit_condition(
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
                            },
                        )),
                    ),
            )
            .child(self.signal_row(
                "Data point",
                entry::term_signal(term),
                Socket::Term(spot),
                cx,
            ))
            .child(panel::row(
                "Condition Trigger",
                Some("Choose when the effect should be triggered.".into()),
                h_flex().flex_wrap().gap_1().children(TIMINGS.map(|choice| {
                    Button::new(SharedString::from(format!(
                        "timing-{}-{}-{choice:?}",
                        spot.group, spot.term
                    )))
                    .label(choice.label())
                    .xsmall()
                    .outline()
                    .selected(timing == choice)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.edit_term(spot, |term| *term = choice.apply(term), window, cx);
                    }))
                })),
                cx,
            ))
            .when_some(comparison_of(term), |this, comparison| {
                this.child(self.comparison_rows(spot, comparison, cx))
            })
            .when(timing != Timing::While, |this| {
                this.child(panel::row(
                    "Show for",
                    Some("Milliseconds, from 1 to 60000.".into()),
                    self.number(Field::Time(spot)),
                    cx,
                ))
            })
    }

    fn comparison_rows(&self, spot: Spot, comparison: Comparison, cx: &Context<Self>) -> Div {
        let constant = matches!(comparison.operand, Operand::Constant(_));
        let operand = match comparison.operand {
            Operand::Constant(_) => {
                panel::row("Value", None, self.number(Field::Constant(spot)), cx)
            }
            Operand::Signal(signal) => {
                self.signal_row("Compared with", signal, Socket::Operand(spot), cx)
            }
        };
        v_flex()
            .gap_1()
            .child(panel::row(
                "Comparison",
                None,
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(comparator_menu(spot, comparison.comparator, cx))
                    .children([("a value", true), ("a data point", false)].map(
                        |(label, value)| {
                            Button::new(SharedString::from(format!(
                                "operand-{}-{}-{value}",
                                spot.group, spot.term
                            )))
                            .label(label)
                            .xsmall()
                            .outline()
                            .selected(constant == value)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    if constant != value {
                                        this.set_operand_kind(spot, value, window, cx);
                                    }
                                },
                            ))
                        },
                    )),
                cx,
            ))
            .child(operand)
    }

    fn edit_condition(
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

    fn set_logic(
        &mut self,
        group: Option<usize>,
        logic: Logic,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit_condition(
            |condition| {
                let Some(condition) = condition else {
                    return;
                };
                match group {
                    None => condition.logic = logic,
                    Some(index) => {
                        if let Some(group) = condition.groups.get_mut(index) {
                            group.logic = logic;
                        }
                    }
                }
            },
            window,
            cx,
        );
    }

    fn set_operand_kind(
        &mut self,
        spot: Spot,
        constant: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let operand = if constant {
            Operand::Constant(1.0)
        } else {
            Operand::Signal(Signal::Builtin(Builtin::ActiveLayer))
        };
        self.edit_term(
            spot,
            |term| {
                if let Term::Compare(comparison) = term {
                    comparison.operand = operand;
                }
            },
            window,
            cx,
        );
    }
}

fn logic_row(
    label: &'static str,
    description: &'static str,
    current: Logic,
    group: Option<usize>,
    cx: &Context<LightbarPage>,
) -> Div {
    let scope = group.map_or_else(|| "condition".to_owned(), |index| format!("group-{index}"));
    panel::row(
        label,
        Some(description.into()),
        h_flex().flex_wrap().gap_1().children(LOGICS.map(|logic| {
            Button::new(SharedString::from(format!("{scope}-logic-{logic:?}")))
                .label(entry::logic_name(logic))
                .xsmall()
                .outline()
                .selected(current == logic)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.set_logic(group, logic, window, cx);
                }))
        })),
        cx,
    )
}

fn comparator_menu(
    spot: Spot,
    current: Comparator,
    cx: &Context<LightbarPage>,
) -> impl IntoElement + use<> {
    let page = cx.entity().downgrade();
    Button::new(SharedString::from(format!(
        "comparator-{}-{}",
        spot.group, spot.term
    )))
    .label(entry::comparator_symbol(current))
    .small()
    .outline()
    .dropdown_caret(true)
    .dropdown_menu(move |menu, _, _| {
        COMPARATORS.iter().fold(menu, |menu, comparator| {
            let page = page.clone();
            let comparator = *comparator;
            menu.item(
                PopupMenuItem::new(comparator_name(comparator))
                    .checked(comparator == current)
                    .on_click(move |_, window, cx| {
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
                    }),
            )
        })
    })
}

const fn comparison_of(term: &Term) -> Option<Comparison> {
    match term {
        Term::Compare(comparison) => Some(*comparison),
        Term::Trigger { .. } => None,
    }
}

const fn comparator_name(comparator: Comparator) -> &'static str {
    match comparator {
        Comparator::Equal => "= equal to",
        Comparator::NotEqual => "≠ not equal to",
        Comparator::Greater => "> greater than",
        Comparator::GreaterOrEqual => "≥ at least",
        Comparator::Less => "< less than",
        Comparator::LessOrEqual => "≤ at most",
    }
}
