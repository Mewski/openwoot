use gpui_kit::{
    App, ClickEvent, Context, Div, IntoElement, ParentElement as _, SharedString, Styled as _,
    Window,
    component::{
        Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lightbar::{
    Comparator, Comparison, Condition, Group, Logic, Operand, Signal, Term,
};

use super::{
    LightbarPage,
    entry::{self, COMPARATORS, Spot, TIMINGS, Timing},
    entry_editor::Socket,
    number::Field,
};
use crate::{
    page::lighting::{add_button, segmented, select},
    panel,
};

const COMPARATOR_WIDTH: f32 = 64.0;

const SHOW: [&str; 2] = ["Always", "Conditional"];
const COMBINE: [(Logic, &str); 2] = [(Logic::All, "All"), (Logic::Any, "Any")];

impl LightbarPage {
    pub(super) fn condition_card(&self, cx: &Context<Self>) -> Option<Div> {
        let entry = self.chosen_entry()?;
        let condition = entry.condition.as_ref();
        Some(
            panel::section("Condition", None, cx)
                .child(panel::toggle(
                    "Show",
                    None,
                    segmented(
                        "condition-show",
                        SHOW,
                        usize::from(condition.is_some()),
                        cx.listener(|this, index: &usize, window, cx| {
                            let conditional = *index == 1;
                            this.edit_condition(
                                |condition| {
                                    if condition.is_some() != conditional {
                                        *condition = conditional.then(entry::default_condition);
                                    }
                                },
                                window,
                                cx,
                            );
                        }),
                    ),
                    cx,
                ))
                .when_some(condition, |this, condition| {
                    this.children(self.condition_rows(condition, cx))
                }),
        )
    }

    fn condition_rows(&self, condition: &Condition, cx: &Context<Self>) -> Vec<Div> {
        let groups = condition.groups.len();
        let mut rows = Vec::new();
        if groups > 1 {
            rows.push(combine_row("condition-logic", condition.logic, None, cx));
        }
        for (index, group) in condition.groups.iter().enumerate() {
            if groups > 1 {
                rows.push(sub_header(
                    format!("Group {}", index + 1),
                    format!("remove-group-{index}"),
                    cx.listener(move |this, _, window, cx| {
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
                    }),
                    cx,
                ));
            }
            rows.extend(self.group_rows(index, group, cx));
        }
        let can_add_group = entry::can_add_group(condition);
        let roomy = condition.groups.iter().rposition(entry::can_add_term);
        rows.push(
            h_flex()
                .w_full()
                .gap_2()
                .when_some(roomy, |this, roomy| {
                    this.child(div().flex_1().child(
                        add_button("add-term", "Add condition", cx).on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.edit_condition(
                                    |condition| {
                                        if let Some(group) = condition
                                            .as_mut()
                                            .and_then(|condition| condition.groups.get_mut(roomy))
                                        {
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
                .when(can_add_group, |this| {
                    this.child(div().flex_1().child(
                        add_button("add-group", "Add group", cx).on_click(cx.listener(
                            |this, _, window, cx| {
                                this.edit_condition(
                                    |condition| {
                                        if let Some(condition) = condition {
                                            entry::add_group(condition);
                                        }
                                    },
                                    window,
                                    cx,
                                );
                            },
                        )),
                    ))
                }),
        );
        rows
    }

    fn group_rows(&self, index: usize, group: &Group, cx: &Context<Self>) -> Vec<Div> {
        let terms = group.terms.len();
        let mut rows = Vec::new();
        if terms > 1 {
            rows.push(combine_row("group-logic", group.logic, Some(index), cx));
        }
        for (term, value) in group.terms.iter().enumerate() {
            let spot = Spot { group: index, term };
            if terms > 1 {
                rows.push(sub_header(
                    format!("Condition {}", term + 1),
                    format!("remove-term-{index}-{term}"),
                    cx.listener(move |this, _, window, cx| {
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
                    }),
                    cx,
                ));
            }
            rows.extend(self.term_rows(spot, value, cx));
        }
        rows
    }

    fn term_rows(&self, spot: Spot, term: &Term, cx: &Context<Self>) -> Vec<Div> {
        let timing = Timing::of(term);
        let mut rows = vec![
            self.signal_field(
                "Data point",
                entry::term_signal(term),
                Socket::Term(spot),
                cx,
            ),
            panel::toggle(
                "Trigger",
                None,
                segmented(
                    SharedString::from(format!("timing-{}-{}", spot.group, spot.term)),
                    TIMINGS.map(Timing::label),
                    TIMINGS
                        .iter()
                        .position(|choice| *choice == timing)
                        .unwrap_or(0),
                    cx.listener(move |this, index: &usize, window, cx| {
                        if let Some(choice) = TIMINGS.get(*index).copied() {
                            this.edit_term(spot, |term| *term = choice.apply(term), window, cx);
                        }
                    }),
                ),
                cx,
            ),
        ];
        if let Term::Compare(comparison) = term {
            rows.push(self.comparison_row(spot, *comparison, cx));
        }
        if timing != Timing::While {
            rows.push(panel::toggle(
                "Duration",
                None,
                self.number(Field::Time(spot), "ms"),
                cx,
            ));
        }
        rows
    }

    fn comparison_row(&self, spot: Spot, comparison: Comparison, cx: &Context<Self>) -> Div {
        let operand = match comparison.operand {
            Operand::Constant(_) => None,
            Operand::Signal(signal) => Some(signal),
        };
        panel::field(
            "Compare",
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(comparator_menu(spot, comparison.comparator, cx))
                .child(div().flex_1().min_w_0().child(self.signal_menu(
                    operand,
                    Socket::Operand(spot),
                    cx,
                )))
                .map(|this| match operand {
                    None => this.child(self.number(Field::Constant(spot), "")),
                    Some(Signal::Host(_)) => this.child(self.number(Field::OperandHost(spot), "")),
                    Some(_) => this,
                }),
            cx,
        )
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
}

fn combine_row(
    id: &'static str,
    current: Logic,
    group: Option<usize>,
    cx: &Context<LightbarPage>,
) -> Div {
    let scope = group.map_or_else(|| id.to_owned(), |index| format!("{id}-{index}"));
    panel::toggle(
        "Combine",
        None,
        segmented(
            SharedString::from(scope),
            COMBINE.map(|(_, label)| label),
            COMBINE
                .iter()
                .position(|(logic, _)| *logic == current)
                .unwrap_or(0),
            cx.listener(move |this, index: &usize, window, cx| {
                if let Some((logic, _)) = COMBINE.get(*index) {
                    this.set_logic(group, *logic, window, cx);
                }
            }),
        ),
        cx,
    )
}

fn sub_header(
    title: String,
    id: String,
    on_remove: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &Context<LightbarPage>,
) -> Div {
    h_flex()
        .w_full()
        .pt_1()
        .justify_between()
        .items_center()
        .child(panel::label(title, cx))
        .child(
            Button::new(SharedString::from(id))
                .label("Remove")
                .ghost()
                .xsmall()
                .on_click(on_remove),
        )
}

fn comparator_menu(
    spot: Spot,
    current: Comparator,
    cx: &Context<LightbarPage>,
) -> impl IntoElement + use<> {
    let page = cx.entity().downgrade();
    select(
        format!("comparator-{}-{}", spot.group, spot.term),
        entry::comparator_symbol(current),
    )
    .w(px(COMPARATOR_WIDTH))
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
