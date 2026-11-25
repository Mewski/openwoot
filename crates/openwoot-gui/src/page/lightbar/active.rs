use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, FontWeight, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, IconName, h_flex, input::Input, tooltip::Tooltip, v_flex},
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
    page::lighting::{ICON_SIZE, LABEL_TEXT, kebab},
    panel,
    switch::Switch,
};

const MENU_WIDTH: f32 = 230.0;
const DRAG_WIDTH: f32 = 352.0;
const ROW_HEIGHT: f32 = 45.0;
const ROW_LINE: f32 = 21.0;
const RADIUS: f32 = 6.0;
const TAB_HEIGHT: f32 = 32.0;
const TAB_TEXT: f32 = 16.0;
const TAB_LINE: f32 = 24.0;
const HEADER_LINE: f32 = 19.2;
const DETAIL_TEXT: f32 = 12.0;
const CHEVRON_SIZE: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Duplicate,
    Rename,
    Transfer,
    Delete,
}

#[derive(Debug, Clone)]
struct EntryDrag {
    owner: Owner,
    index: usize,
    kind: Kind,
    name: SharedString,
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

    const fn title(self) -> &'static str {
        match self {
            Self::Profile => "Profile Effects",
            Self::Global => "Global Effects",
        }
    }
}

impl Action {
    const fn label(self, owner: Owner) -> &'static str {
        match (self, owner) {
            (Self::Duplicate, _) => "Duplicate",
            (Self::Rename, _) => "Rename",
            (Self::Transfer, Owner::Profile) => "Move to Global Effects",
            (Self::Transfer, Owner::Global) => "Move to Profile Effects",
            (Self::Delete, _) => "Delete",
        }
    }
}

impl LightbarPage {
    pub(super) fn active_card(&self, cx: &Context<Self>) -> Option<Div> {
        let settings = self.settings.as_ref()?;
        let owner = self.owner;
        let entries = self.entries(owner)?;
        let theme = cx.theme();
        let mut add = (self.tab == Tab::Creator && entries.len() < MAX_ENTRIES).then(|| {
            panel::ActionButton::new(format!("{}-add", owner.key()), "Add New Effect")
                .large()
                .icon(Icon::new(IconName::Plus))
                .w_full()
                .on_click(cx.listener(move |this, _, window, cx| {
                    let entry = Entry {
                        title: entry::title(Kind::Solid.name()),
                        condition: None,
                        effect: Kind::Solid.effect(this.matrix(cx)),
                    };
                    this.add_entry(owner, entry, window, cx);
                }))
        });
        let runs = runs(entries);
        let list = if runs.is_empty() {
            v_flex().w_full().gap_2().child(
                run_box(cx)
                    .gap_6()
                    .items_center()
                    .children(add.take())
                    .child(
                        div()
                            .text_size(px(TAB_TEXT))
                            .line_height(px(TAB_LINE))
                            .text_color(theme.foreground)
                            .child(owner.empty()),
                    ),
            )
        } else {
            v_flex()
                .w_full()
                .gap_2()
                .children(runs.into_iter().map(|(temporary, indices)| {
                    run_box(cx).gap_6().children(add.take()).child(
                        v_flex()
                            .w_full()
                            .gap_3()
                            .child(panel::label(run_name(temporary), cx))
                            .child(v_flex().w_full().gap_1().children(
                                indices.into_iter().filter_map(|index| {
                                    entries.get(index).map(|entry| {
                                        self.entry_pill(owner, index, entry, entries.len(), cx)
                                    })
                                }),
                            )),
                    )
                }))
        };
        Some(
            panel::section_with_help(
                "Active Effects",
                "Profile effects apply to this profile and global effects to every profile.",
                None,
                None,
                cx,
            )
            .p_6()
            .child(
                v_flex()
                    .w_full()
                    .gap_3()
                    .child(Self::owner_tabs(owner, cx))
                    .child(list.when(owner == Owner::Profile, |this| {
                        this.child(self.inherit_section(settings.use_global, cx))
                    })),
            ),
        )
    }

    fn owner_tabs(owner: Owner, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        h_flex()
            .w_full()
            .p_1()
            .gap_1()
            .rounded(px(RADIUS))
            .bg(theme.accent)
            .children([Owner::Profile, Owner::Global].map(|choice| {
                let selected = choice == owner;
                div()
                    .id(SharedString::from(format!("active-owner-{}", choice.key())))
                    .flex_1()
                    .h(px(TAB_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_3()
                    .rounded(px(RADIUS))
                    .text_size(px(TAB_TEXT))
                    .line_height(px(TAB_LINE))
                    .cursor_pointer()
                    .map(|this| {
                        if selected {
                            this.bg(theme.accent).text_color(theme.foreground)
                        } else {
                            this.text_color(theme.muted_foreground)
                                .hover(|style| style.text_color(theme.foreground))
                        }
                    })
                    .child(choice.title())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.owner = choice;
                        cx.notify();
                    }))
            }))
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
            kind,
            name: SharedString::from(entry::entry_name(entry).to_owned()),
        };
        let name = if self.renaming == Some(chosen) {
            div()
                .w_full()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.renaming = None;
                        cx.notify();
                    }
                }))
                .child(
                    Input::new(&self.rename)
                        .appearance(false)
                        .px_0()
                        .text_size(px(LABEL_TEXT))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .into_any_element()
        } else {
            name_text(entry::entry_name(entry).to_owned(), cx).into_any_element()
        };
        row(
            SharedString::from(format!("{}-entry-{index}", owner.key())),
            kind,
            name,
            detail,
            self.chosen == Some(chosen),
            cx,
        )
        .cursor_pointer()
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
        let actions = [
            Action::Duplicate,
            Action::Rename,
            Action::Transfer,
            Action::Delete,
        ];
        let items = actions.map(|action| {
            let blocked = match action {
                Action::Duplicate => count >= MAX_ENTRIES,
                Action::Transfer => other_full,
                Action::Rename | Action::Delete => false,
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
        let profile_full = self
            .settings
            .as_ref()
            .is_none_or(|settings| settings.entries.len() >= MAX_ENTRIES);
        let header = h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .id("inherit-toggle")
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .text_size(px(TAB_TEXT))
                    .line_height(px(HEADER_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(
                        Icon::new(if open {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .size(px(CHEVRON_SIZE)),
                    )
                    .child("Inherit Global Effects")
                    .child(
                        div()
                            .id("inherit-help")
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(Catalog::CircleQuestionMark).size(px(ICON_SIZE)))
                            .tooltip(|window, cx| {
                                Tooltip::new("When enabled, the global effects apply alongside this profile's own effects.")
                                    .build(window, cx)
                            }),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.inherit_open = !this.inherit_open;
                        cx.notify();
                    })),
            )
            .child(
                Switch::new("inherit-global")
                    .checked(inherit)
                    .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                        let enabled = *enabled;
                        this.edit_settings(|settings| settings.use_global = enabled, window, cx);
                    })),
            );
        let global = self.global.as_ref();
        let content = global.filter(|global| !global.is_empty()).map_or_else(
            || {
                div()
                    .text_size(px(LABEL_TEXT))
                    .text_color(theme.muted_foreground)
                    .child(Owner::Global.empty())
            },
            |global| {
                v_flex()
                    .w_full()
                    .gap_6()
                    .children(runs(global).into_iter().map(|(temporary, indices)| {
                        v_flex()
                            .w_full()
                            .gap_3()
                            .child(panel::label(run_name(temporary), cx))
                            .child(v_flex().w_full().gap_1().children(
                                indices.into_iter().filter_map(|index| {
                                    global.get(index).map(|entry| {
                                        Self::inherited_row(index, entry, inherit, profile_full, cx)
                                    })
                                }),
                            ))
                    }))
            },
        );
        run_box(cx).child(header).when(open, |this| {
            this.child(div().w_full().pt_6().child(content))
        })
    }

    fn inherited_row(
        index: usize,
        entry: &Entry,
        inherit: bool,
        profile_full: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let kind = Kind::of(&entry.effect);
        row(
            SharedString::from(format!("inherited-{index}")),
            kind,
            name_text(entry::entry_name(entry).to_owned(), cx).into_any_element(),
            None,
            false,
            cx,
        )
        .when(!inherit, |this| this.opacity(0.5))
        .child(
            panel::ActionButton::icon_only(
                SharedString::from(format!("copy-global-{index}")),
                Icon::new(IconName::Copy),
            )
            .disabled(profile_full)
            .tooltip("Copy to Profile Effects")
            .on_click(cx.listener(move |this, _, window, cx| {
                let copied = this
                    .global
                    .as_ref()
                    .and_then(|global| global.get(index))
                    .cloned();
                if let Some(entry) = copied {
                    this.add_entry(Owner::Profile, entry, window, cx);
                }
            })),
        )
    }

    pub(super) fn finish_rename(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Chosen { owner, index }) = self.renaming.take() else {
            return;
        };
        let title = entry::title(text.trim());
        let changed = self
            .entries(owner)
            .and_then(|entries| entries.get(index))
            .is_some_and(|entry| entry.title != title);
        if changed {
            self.edit_entries(
                owner,
                |entries| {
                    if let Some(entry) = entries.get_mut(index) {
                        entry.title = title;
                    }
                },
                window,
                cx,
            );
        }
        cx.notify();
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
            Action::Rename => {
                let name = entry::entry_name(&entry).to_owned();
                self.renaming = Some(Chosen { owner, index });
                self.rename.update(cx, |state, cx| {
                    state.set_value(name, window, cx);
                    state.focus(window, cx);
                    state.select_all(window, cx);
                });
                cx.notify();
            }
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

fn runs(entries: &Entries) -> Vec<(bool, Vec<usize>)> {
    let mut runs: Vec<(bool, Vec<usize>)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let temporary = entry.condition.is_some();
        match runs.last_mut() {
            Some((kind, indices)) if *kind == temporary => indices.push(index),
            _ => runs.push((temporary, vec![index])),
        }
    }
    runs
}

const fn run_name(temporary: bool) -> &'static str {
    if temporary { "Temporary" } else { "Constant" }
}

fn run_box(cx: &App) -> Div {
    v_flex()
        .w_full()
        .p_4()
        .rounded(px(RADIUS))
        .bg(cx.theme().tab_active)
}

fn row(
    id: SharedString,
    kind: Kind,
    name: AnyElement,
    detail: Option<SharedString>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    h_flex()
        .id(id)
        .w_full()
        .min_h(px(ROW_HEIGHT))
        .pl_4()
        .pr_1p5()
        .gap_2()
        .items_center()
        .rounded(px(RADIUS))
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.transparent
        })
        .bg(theme.accent)
        .child(
            div()
                .flex_none()
                .text_color(theme.foreground)
                .child(Icon::new(kind.icon()).size(px(ICON_SIZE))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .py_1()
                .child(name)
                .children(detail.map(|detail| {
                    div()
                        .truncate()
                        .text_size(px(DETAIL_TEXT))
                        .text_color(theme.muted_foreground)
                        .child(detail)
                })),
        )
}

fn name_text(name: String, cx: &App) -> Div {
    div()
        .truncate()
        .text_size(px(LABEL_TEXT))
        .line_height(px(ROW_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().group_box_foreground)
        .child(name)
}

impl Render for EntryDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(DRAG_WIDTH)).shadow_lg().child(row(
            SharedString::from("entry-drag"),
            self.kind,
            name_text(self.name.to_string(), cx).into_any_element(),
            None,
            true,
            cx,
        ))
    }
}
