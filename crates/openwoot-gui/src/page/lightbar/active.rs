use gpui_kit::{
    AppContext as _, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::lightbar::{Entries, Entry, MAX_ENTRIES};

use super::{
    Chosen, LightbarPage, Owner, Tab,
    entry::{self, Kind},
};
use crate::{
    dropdown::{Dropdown, Item},
    page::lighting::{Choice, add_button, choices, kebab, pill},
    panel,
    switch::Switch,
};

const MENU_WIDTH: f32 = 230.0;
const DRAG_WIDTH: f32 = 240.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Duplicate,
    Transfer,
    Delete,
}

#[derive(Debug, Clone)]
struct EntryDrag {
    owner: Owner,
    index: usize,
    name: &'static str,
    icon: Catalog,
}

impl Owner {
    const fn key(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Global => "global",
        }
    }

    const fn other(self) -> Self {
        match self {
            Self::Profile => Self::Global,
            Self::Global => Self::Profile,
        }
    }

    const fn empty(self) -> &'static str {
        match self {
            Self::Profile => "No profile effects active",
            Self::Global => "No global effects active",
        }
    }
}

impl Action {
    const fn label(self, owner: Owner) -> &'static str {
        match (self, owner) {
            (Self::Duplicate, _) => "Duplicate",
            (Self::Transfer, Owner::Profile) => "Move to global effects",
            (Self::Transfer, Owner::Global) => "Move to profile effects",
            (Self::Delete, _) => "Delete",
        }
    }
}

impl LightbarPage {
    pub(super) fn active_card(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let owner = self.owner;
        let entries = self.entries(owner)?;
        let tabs = choices(
            "active-owner",
            ["Profile effects", "Global effects"],
            Some(usize::from(owner == Owner::Global)),
            Choice::Outlined,
            cx.listener(|this, index: &usize, _, cx| {
                this.owner = if *index == 1 {
                    Owner::Global
                } else {
                    Owner::Profile
                };
                cx.notify();
            }),
            cx,
        );
        let list = v_flex()
            .w_full()
            .gap_2()
            .p_3()
            .rounded(px(8.0))
            .bg(cx.theme().background)
            .when(
                self.tab == Tab::Creator && entries.len() < MAX_ENTRIES,
                |this| {
                    this.child(
                        add_button(format!("{}-add", owner.key()), "Add new effect", cx).on_click(
                            cx.listener(move |this, _, window, cx| {
                                let entry = Entry {
                                    condition: None,
                                    effect: Kind::Solid.effect(this.matrix(cx)),
                                };
                                this.add_entry(owner, entry, window, cx);
                            }),
                        ),
                    )
                },
            )
            .children(self.entry_rows(owner, entries, cx))
            .when(entries.is_empty(), |this| {
                this.child(
                    div()
                        .w_full()
                        .py_3()
                        .text_center()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(owner.empty()),
                )
            });
        Some(
            panel::section("Active effects", None, cx)
                .child(tabs)
                .child(list)
                .when(owner == Owner::Profile, |this| {
                    this.child(self.inherit_section(settings.use_global, cx))
                }),
        )
    }

    fn entry_rows(&self, owner: Owner, entries: &Entries, cx: &Context<Self>) -> Vec<Div> {
        let mut rows = Vec::new();
        let mut last_kind = None;
        for (index, entry) in entries.iter().enumerate() {
            let temporary = entry.condition.is_some();
            if last_kind != Some(temporary) {
                rows.push(div().pt_1().child(panel::label(
                    if temporary { "Temporary" } else { "Constant" },
                    cx,
                )));
                last_kind = Some(temporary);
            }
            rows.push(div().child(self.entry_pill(owner, index, entry, entries.len(), cx)));
        }
        rows
    }

    fn entry_pill(
        &self,
        owner: Owner,
        index: usize,
        entry: &Entry,
        count: usize,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let chosen = Chosen { owner, index };
        let kind = Kind::of(&entry.effect);
        let detail = entry
            .condition
            .as_ref()
            .map(|condition| SharedString::from(entry::condition_summary(condition)));
        let primary = cx.theme().primary;
        let drag = EntryDrag {
            owner,
            index,
            name: kind.name(),
            icon: kind.icon(),
        };
        pill(
            SharedString::from(format!("{}-entry-{index}", owner.key())),
            Some(kind.icon()),
            kind.name(),
            detail,
            self.chosen == Some(chosen),
            cx,
        )
        .on_click(cx.listener(move |this, _, window, cx| this.choose(chosen, window, cx)))
        .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
        .drag_over::<EntryDrag>(move |style, drag, _, _| {
            if drag.owner == owner {
                style.border_color(primary)
            } else {
                style
            }
        })
        .on_drop(cx.listener(move |this, drag: &EntryDrag, window, cx| {
            if drag.owner == owner {
                this.move_entry(owner, drag.index, index, window, cx);
            }
        }))
        .child(self.entry_menu(owner, index, count, cx))
    }

    fn entry_menu(
        &self,
        owner: Owner,
        index: usize,
        count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let page = cx.entity().downgrade();
        let other_full = self
            .entries(owner.other())
            .is_none_or(|other| other.len() >= MAX_ENTRIES);
        let actions = [Action::Duplicate, Action::Transfer, Action::Delete];
        let items = actions.map(|action| {
            let blocked = match action {
                Action::Duplicate => count >= MAX_ENTRIES,
                Action::Transfer => other_full,
                Action::Delete => false,
            };
            Item::new(action.label(owner))
                .disabled(blocked)
                .danger(action == Action::Delete)
        });
        div().flex_none().child(
            Dropdown::new(SharedString::from(format!(
                "{}-entry-menu-{index}",
                owner.key()
            )))
            .trigger(kebab)
            .align_end()
            .menu_width(px(MENU_WIDTH))
            .items(items)
            .on_select(move |position, window, cx| {
                let Some(action) = actions.get(position).copied() else {
                    return;
                };
                page.update(cx, |page, cx| page.act(owner, index, action, window, cx))
                    .ok();
            }),
        )
    }

    fn inherit_section(&self, inherit: bool, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let open = self.inherit_open;
        let global = self.global.as_ref();
        let profile_full = self
            .settings
            .as_ref()
            .is_none_or(|settings| settings.entries.len() >= MAX_ENTRIES);
        v_flex()
            .w_full()
            .gap_2()
            .p_3()
            .rounded(px(8.0))
            .bg(theme.background)
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .id("inherit-toggle")
                            .flex()
                            .flex_1()
                            .gap_2()
                            .items_center()
                            .cursor_pointer()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(
                                Icon::new(if open {
                                    IconName::ChevronUp
                                } else {
                                    IconName::ChevronDown
                                })
                                .small(),
                            )
                            .child("Inherit global effects")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.inherit_open = !this.inherit_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("inherit-help")
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(Catalog::CircleQuestionMark).xsmall())
                            .tooltip(|window, cx| {
                                Tooltip::new("Global effects show under this profile's own effects")
                                    .build(window, cx)
                            }),
                    )
                    .child(
                        Switch::new("inherit-global")
                            .checked(inherit)
                            .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                                let enabled = *enabled;
                                this.edit_settings(
                                    |settings| settings.use_global = enabled,
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
            .when(open, |this| {
                let rows: Vec<Div> = global.map_or_else(Vec::new, |global| {
                    global
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| {
                            Self::inherited_row(index, entry, inherit, profile_full, cx)
                        })
                        .collect()
                });
                if rows.is_empty() {
                    this.child(panel::note(Owner::Global.empty(), cx))
                } else {
                    this.children(rows)
                }
            })
    }

    fn inherited_row(
        index: usize,
        entry: &Entry,
        inherit: bool,
        profile_full: bool,
        cx: &Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let kind = Kind::of(&entry.effect);
        h_flex()
            .w_full()
            .min_h(px(40.0))
            .pl_4()
            .pr_2()
            .gap_3()
            .items_center()
            .rounded(px(6.0))
            .bg(theme.secondary)
            .text_sm()
            .text_color(if inherit {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .child(Icon::new(kind.icon()).small())
            .child(div().flex_1().min_w_0().truncate().child(kind.name()))
            .child(
                div()
                    .id(SharedString::from(format!("copy-global-{index}")))
                    .size(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .text_color(theme.muted_foreground)
                    .when(!profile_full, |this| {
                        this.cursor_pointer()
                            .hover(|style| style.bg(theme.secondary_hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let copied = this
                                    .global
                                    .as_ref()
                                    .and_then(|global| global.get(index))
                                    .cloned();
                                if let Some(entry) = copied {
                                    this.add_entry(Owner::Profile, entry, window, cx);
                                }
                            }))
                    })
                    .child(Icon::new(IconName::Copy).small())
                    .tooltip(|window, cx| {
                        Tooltip::new("Copy to profile effects").build(window, cx)
                    }),
            )
    }

    pub(super) fn choose(&mut self, chosen: Chosen, window: &mut Window, cx: &mut Context<Self>) {
        self.chosen = Some(chosen);
        self.custom_source = false;
        self.owner = chosen.owner;
        self.led = 0;
        self.define = true;
        self.sync_entry_controls(window, cx);
        cx.notify();
    }

    pub(super) fn add_entry(
        &mut self,
        owner: Owner,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(count) = self.entries(owner).map(|entries| entries.len()) else {
            return;
        };
        if count >= MAX_ENTRIES {
            return;
        }
        self.edit_entries(
            owner,
            |entries| {
                entries.push(entry).ok();
            },
            window,
            cx,
        );
        self.choose(
            Chosen {
                owner,
                index: count,
            },
            window,
            cx,
        );
    }

    fn act(
        &mut self,
        owner: Owner,
        index: usize,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .entries(owner)
            .and_then(|entries| entries.get(index))
            .cloned()
        else {
            return;
        };
        match action {
            Action::Duplicate => {
                let mut inserted = false;
                self.edit_entries(
                    owner,
                    |entries| inserted = entries.insert(index + 1, entry).is_ok(),
                    window,
                    cx,
                );
                if inserted {
                    self.choose(
                        Chosen {
                            owner,
                            index: index + 1,
                        },
                        window,
                        cx,
                    );
                }
            }
            Action::Transfer => {
                let other = owner.other();
                if self
                    .entries(other)
                    .is_none_or(|entries| entries.len() >= MAX_ENTRIES)
                {
                    return;
                }
                self.edit_entries(
                    owner,
                    |entries| {
                        entries.remove(index);
                    },
                    window,
                    cx,
                );
                self.add_entry(other, entry, window, cx);
            }
            Action::Delete => {
                self.edit_entries(
                    owner,
                    |entries| {
                        entries.remove(index);
                    },
                    window,
                    cx,
                );
                self.chosen = None;
                cx.notify();
            }
        }
    }

    fn move_entry(
        &mut self,
        owner: Owner,
        from: usize,
        to: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.entries(owner).map_or(0, |entries| entries.len());
        if from == to || from >= count || to >= count {
            return;
        }
        self.edit_entries(
            owner,
            |entries| {
                let moved = entries.remove(from);
                entries.insert(to, moved).ok();
            },
            window,
            cx,
        );
        self.choose(Chosen { owner, index: to }, window, cx);
    }
}

impl Render for EntryDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .w(px(DRAG_WIDTH))
            .h(px(46.0))
            .px_4()
            .gap_3()
            .items_center()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.primary)
            .bg(theme.background)
            .shadow_lg()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.foreground)
            .child(Icon::new(self.icon).small())
            .child(self.name)
    }
}
