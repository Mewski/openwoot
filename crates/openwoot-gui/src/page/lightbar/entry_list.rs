use gpui_kit::{
    App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};
use openwoot_protocol::lightbar::{Entry, MAX_ENTRIES};

use super::{
    Chosen, LightbarPage, Owner,
    entry::{self, KINDS, Kind},
    preset::PRESETS,
};
use crate::panel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Up,
    Down,
    Duplicate,
    Transfer,
    Delete,
}

impl Owner {
    const fn title(self) -> &'static str {
        match self {
            Self::Profile => "Profile effects",
            Self::Global => "Global effects",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Profile => "Shown while this profile is active.",
            Self::Global => {
                "Shared by every profile that inherits them, drawn below the profile's own \
                 effects. Save keeps them for the whole keyboard."
            }
        }
    }

    const fn empty(self) -> &'static str {
        match self {
            Self::Profile => "No profile effects active",
            Self::Global => "No global effects active",
        }
    }

    const fn full(self) -> &'static str {
        match self {
            Self::Profile => "You've reached the maximum number of effects!",
            Self::Global => "You've reached the maximum number of global effects!",
        }
    }

    const fn key(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Global => "global",
        }
    }

    pub(super) const fn other(self) -> Self {
        match self {
            Self::Profile => Self::Global,
            Self::Global => Self::Profile,
        }
    }
}

impl Move {
    const ALL: [Self; 5] = [
        Self::Up,
        Self::Down,
        Self::Duplicate,
        Self::Transfer,
        Self::Delete,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Duplicate => "duplicate",
            Self::Transfer => "transfer",
            Self::Delete => "delete",
        }
    }

    const fn tooltip(self, owner: Owner) -> &'static str {
        match (self, owner) {
            (Self::Up, _) => "Move up",
            (Self::Down, _) => "Move down",
            (Self::Duplicate, _) => "Duplicate",
            (Self::Transfer, Owner::Profile) => "Move to Global Effects",
            (Self::Transfer, Owner::Global) => "Move to Profile Effects",
            (Self::Delete, _) => "Delete",
        }
    }
}

impl LightbarPage {
    pub(super) fn list_section(&self, owner: Owner, cx: &Context<Self>) -> Div {
        let entries = self.entries(owner);
        let count = entries.map_or(0, |entries| entries.len());
        let full = count >= MAX_ENTRIES;
        panel::section(owner.title(), Some(owner.description().into()), cx)
            .child(
                v_flex().gap_1().children(
                    entries
                        .into_iter()
                        .flat_map(|entries| entries.iter().enumerate())
                        .map(|(index, entry)| self.entry_row(owner, index, entry, count, cx)),
                ),
            )
            .when(entries.is_some() && count == 0, |this| {
                this.child(panel::note(owner.empty(), cx))
            })
            .when(full, |this| this.child(panel::note(owner.full(), cx)))
            .child(
                h_flex()
                    .gap_2()
                    .child(self.add_menu(owner, true, entries.is_none() || full, cx))
                    .child(self.add_menu(owner, false, entries.is_none() || full, cx)),
            )
    }

    fn entry_row(
        &self,
        owner: Owner,
        index: usize,
        entry: &Entry,
        count: usize,
        cx: &Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let chosen = Chosen { owner, index };
        let selected = self.chosen == Some(chosen);
        let condition = entry
            .condition
            .as_ref()
            .map_or_else(|| "Always".to_owned(), entry::condition_summary);
        let kind = Kind::of(&entry.effect);
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .px_3()
            .py_1()
            .rounded(theme.radius)
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.border
            })
            .child(
                h_flex()
                    .id(SharedString::from(format!("{}-entry-{index}", owner.key())))
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose(chosen, window, cx);
                    }))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}", index + 1)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(kind.name()),
                    )
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .child(entry::effect_summary(&entry.effect)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(condition),
                            ),
                    ),
            )
            .children(Move::ALL.map(|step| {
                let blocked = match step {
                    Move::Up => index == 0,
                    Move::Down => index + 1 == count,
                    Move::Duplicate => count >= MAX_ENTRIES,
                    Move::Transfer => self
                        .entries(owner.other())
                        .is_none_or(|other| other.len() >= MAX_ENTRIES),
                    Move::Delete => false,
                };
                let icon: Icon = match step {
                    Move::Up => IconName::ArrowUp.into(),
                    Move::Down => IconName::ArrowDown.into(),
                    Move::Duplicate => IconName::Copy.into(),
                    Move::Transfer => Catalog::ArrowUpDown.into(),
                    Move::Delete => IconName::Delete.into(),
                };
                Button::new(SharedString::from(format!(
                    "{}-entry-{}-{index}",
                    owner.key(),
                    step.key()
                )))
                .icon(icon)
                .xsmall()
                .ghost()
                .tooltip(step.tooltip(owner))
                .disabled(blocked)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.move_entry(owner, index, step, window, cx);
                }))
            }))
    }

    fn add_menu(
        &self,
        owner: Owner,
        presets: bool,
        disabled: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let matrix = self.matrix(cx);
        let (id, label, icon) = if presets {
            ("presets", "Effect presets", Catalog::Sparkles)
        } else {
            ("blank", "Add new effect", Catalog::Plus)
        };
        Button::new(SharedString::from(format!("{}-add-{id}", owner.key())))
            .icon(icon)
            .label(label)
            .small()
            .outline()
            .dropdown_caret(true)
            .disabled(disabled)
            .dropdown_menu(move |menu, _, _| {
                let add = |entry: Entry| {
                    let page = page.clone();
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        page.update(cx, |page, cx| {
                            page.add_entry(owner, entry.clone(), window, cx);
                        })
                        .ok();
                    }
                };
                if presets {
                    PRESETS.iter().fold(menu, |menu, preset| {
                        let (name, description) = (preset.name, preset.description);
                        menu.item(
                            PopupMenuItem::element(move |_, cx| {
                                v_flex().py_0p5().child(div().text_sm().child(name)).child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(description),
                                )
                            })
                            .on_click(add(preset.entry(matrix))),
                        )
                    })
                } else {
                    KINDS.iter().fold(menu, |menu, kind| {
                        menu.item(PopupMenuItem::new(kind.name()).on_click(add(Entry {
                            condition: None,
                            effect: kind.effect(matrix),
                        })))
                    })
                }
            })
    }

    pub(super) fn choose(&mut self, chosen: Chosen, window: &mut Window, cx: &mut Context<Self>) {
        self.chosen = Some(chosen);
        self.led = 0;
        self.sync_entry_controls(window, cx);
        cx.notify();
    }

    fn add_entry(
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

    fn move_entry(
        &mut self,
        owner: Owner,
        index: usize,
        step: Move,
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
        let count = self.entries(owner).map_or(0, |entries| entries.len());
        let chosen = match step {
            Move::Up | Move::Down => {
                let target = if step == Move::Up {
                    index.checked_sub(1)
                } else {
                    Some(index + 1).filter(|target| *target < count)
                };
                let Some(target) = target else {
                    return;
                };
                self.edit_entries(owner, |entries| entries.swap(index, target), window, cx);
                Some(Chosen {
                    owner,
                    index: target,
                })
            }
            Move::Duplicate => {
                let mut inserted = false;
                self.edit_entries(
                    owner,
                    |entries| inserted = entries.insert(index + 1, entry).is_ok(),
                    window,
                    cx,
                );
                inserted.then_some(Chosen {
                    owner,
                    index: index + 1,
                })
            }
            Move::Transfer => self.transfer_entry(owner, index, entry, window, cx),
            Move::Delete => {
                self.edit_entries(
                    owner,
                    |entries| {
                        entries.remove(index);
                    },
                    window,
                    cx,
                );
                None
            }
        };
        self.chosen = chosen;
        self.led = 0;
        self.sync_entry_controls(window, cx);
        cx.notify();
    }

    fn transfer_entry(
        &mut self,
        owner: Owner,
        index: usize,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Chosen> {
        let other = owner.other();
        let target = self.entries(other).map(|entries| entries.len())?;
        if target >= MAX_ENTRIES {
            return None;
        }
        self.edit_entries(
            owner,
            |entries| {
                entries.remove(index);
            },
            window,
            cx,
        );
        self.edit_entries(
            other,
            |entries| {
                entries.push(entry).ok();
            },
            window,
            cx,
        );
        Some(Chosen {
            owner: other,
            index: target,
        })
    }
}
