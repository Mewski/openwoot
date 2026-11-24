use gpui_kit::{
    App, ClickEvent, Context, Div, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{
        IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex,
    },
    prelude::FluentBuilder as _,
};
use openwoot_protocol::lightbar::{Entry, MAX_ENTRIES};

use super::{
    Chosen, LightbarPage, Owner,
    entry::{self, Kind},
    preset::PRESETS,
};
use crate::{
    page::lighting::{add_button, pill},
    panel,
};

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
            Self::Profile => "Profile",
            Self::Global => "Global",
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
    const fn label(self, owner: Owner) -> &'static str {
        match (self, owner) {
            (Self::Up, _) => "Move up",
            (Self::Down, _) => "Move down",
            (Self::Duplicate, _) => "Duplicate",
            (Self::Transfer, Owner::Profile) => "Move to global",
            (Self::Transfer, Owner::Global) => "Move to profile",
            (Self::Delete, _) => "Delete",
        }
    }
}

impl LightbarPage {
    pub(super) fn effects_card(&self, cx: &Context<Self>) -> Option<Div> {
        self.settings.as_ref()?;
        Some(
            panel::section(
                "Effects",
                Some("Global effects draw first; later effects draw on top.".into()),
                cx,
            )
            .children(self.effect_group(Owner::Profile, cx))
            .children(self.effect_group(Owner::Global, cx)),
        )
    }

    fn effect_group(&self, owner: Owner, cx: &Context<Self>) -> Option<Div> {
        let entries = self.entries(owner)?;
        let count = entries.len();
        Some(panel::field(
            owner.title(),
            v_flex()
                .w_full()
                .gap_2()
                .children(
                    entries
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| self.entry_pill(owner, index, entry, count, cx)),
                )
                .when(count < MAX_ENTRIES, |this| {
                    this.child(self.add_menu(owner, cx))
                }),
            cx,
        ))
    }

    fn entry_pill(
        &self,
        owner: Owner,
        index: usize,
        entry: &Entry,
        count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let chosen = Chosen { owner, index };
        let mut summary = entry::effect_summary(&entry.effect);
        if let Some(condition) = &entry.condition {
            summary = format!("{summary} · {}", entry::condition_summary(condition));
        }
        pill(
            format!("{}-entry-{index}", owner.key()),
            Kind::of(&entry.effect).name(),
            Some(summary.into()),
            self.chosen == Some(chosen),
            false,
            cx,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            this.choose(chosen, window, cx);
        }))
        .child(self.entry_menu(owner, index, count, cx))
    }

    fn entry_menu(
        &self,
        owner: Owner,
        index: usize,
        count: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let other_full = self
            .entries(owner.other())
            .is_none_or(|other| other.len() >= MAX_ENTRIES);
        let blocked = move |step: Move| match step {
            Move::Up => index == 0,
            Move::Down => index + 1 >= count,
            Move::Duplicate => count >= MAX_ENTRIES,
            Move::Transfer => other_full,
            Move::Delete => false,
        };
        Button::new(SharedString::from(format!(
            "{}-entry-menu-{index}",
            owner.key()
        )))
        .icon(IconName::EllipsisVertical)
        .ghost()
        .xsmall()
        .dropdown_menu(move |menu, _, _| {
            let item = |step: Move| {
                let page = page.clone();
                PopupMenuItem::new(step.label(owner))
                    .disabled(blocked(step))
                    .on_click(move |_, window, cx| {
                        page.update(cx, |page, cx| {
                            page.move_entry(owner, index, step, window, cx);
                        })
                        .ok();
                    })
            };
            menu.item(item(Move::Up))
                .item(item(Move::Down))
                .item(item(Move::Duplicate))
                .item(item(Move::Transfer))
                .separator()
                .item(item(Move::Delete))
        })
    }

    fn add_menu(&self, owner: Owner, cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        let matrix = self.matrix(cx);
        add_button(format!("{}-add", owner.key()), "Add effect", cx).dropdown_menu(
            move |menu, _, _| {
                let add = |entry: Entry| {
                    let page = page.clone();
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        page.update(cx, |page, cx| {
                            page.add_entry(owner, entry.clone(), window, cx);
                        })
                        .ok();
                    }
                };
                PRESETS
                    .iter()
                    .fold(menu, |menu, preset| {
                        menu.item(
                            PopupMenuItem::new(preset.name).on_click(add(preset.entry(matrix))),
                        )
                    })
                    .separator()
                    .item(PopupMenuItem::new(Kind::Custom.name()).on_click(add(Entry {
                        condition: None,
                        effect: Kind::Custom.effect(matrix),
                    })))
            },
        )
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
