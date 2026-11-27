use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window,
    component::{
        ActiveTheme as _, Icon, IconName, WindowExt as _, h_flex, notification::Notification,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    endpoints::GetMacro,
    macros::{Macro, Summary},
};

use super::{KeyTarget, MacrosPage, edit, mode_title, step_count};
use crate::{
    dropdown::{Dropdown, Item},
    page::{lighting::kebab, profiles::count_chip},
    panel,
    store::Library,
};

const MENU_WIDTH: f32 = 200.0;
const ROW_HEIGHT: f32 = 52.0;
const ROW_RADIUS: f32 = 6.0;
const ROW_LINE: f32 = 19.0;
const NAME_TEXT: f32 = 14.0;
const DETAIL_TEXT: f32 = 12.0;
const BADGE_HEIGHT: f32 = 22.0;
const BADGE_WIDTH: f32 = 34.0;
const BADGE_RADIUS: f32 = 4.0;
const LIBRARY_HELP: &str = "Macros are stored on the keyboard and shared by all profiles.";
const EMPTY: &str = "No macros yet";
const NO_ROOM: &str = "All 32 macro slots are in use";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Rename,
    Duplicate,
    Delete,
}

impl Command {
    const ALL: [Self; 3] = [Self::Rename, Self::Duplicate, Self::Delete];

    const fn label(self) -> &'static str {
        match self {
            Self::Rename => "Rename",
            Self::Duplicate => "Duplicate",
            Self::Delete => "Delete",
        }
    }
}

impl MacrosPage {
    pub(super) fn library_card(&self, library: &Library, cx: &mut Context<Self>) -> Div {
        let recording = self.recorder.is_some();
        let full = library.free().is_none();
        let rows: Vec<Stateful<Div>> = library
            .iter()
            .map(|(index, summary)| self.library_row(index, summary, full, cx))
            .collect();
        let list = if rows.is_empty() {
            div()
                .w_full()
                .py_4()
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(EMPTY)
        } else {
            v_flex().w_full().gap_1().children(rows)
        };
        panel::section_with_inline(
            "My Macros",
            count_chip(library.len(), library.capacity(), cx),
            cx,
        )
        .child(panel::description(LIBRARY_HELP, cx))
        .child(list)
        .child(
            panel::ActionButton::new("macro-create", "Create Macro")
                .large()
                .icon(Icon::new(IconName::Plus))
                .w_full()
                .disabled(full || recording)
                .when(full, |this| this.tooltip(NO_ROOM))
                .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
        )
    }

    fn library_row(
        &self,
        index: u8,
        summary: &Summary,
        full: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let selected = self.selected == Some(index);
        let (background, hover, border) = (
            theme.accent,
            theme.secondary,
            if selected {
                theme.primary
            } else {
                theme.transparent
            },
        );
        let detail = format!(
            "{} · {}",
            mode_title(summary.mode),
            step_count(usize::from(summary.steps))
        );
        h_flex()
            .id(SharedString::from(format!("macro-{index}")))
            .w_full()
            .min_h(px(ROW_HEIGHT))
            .pl_3()
            .pr_1p5()
            .gap_3()
            .items_center()
            .rounded(px(ROW_RADIUS))
            .border_1()
            .border_color(border)
            .bg(background)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(
                div()
                    .flex_none()
                    .w(px(BADGE_WIDTH))
                    .h(px(BADGE_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(BADGE_RADIUS))
                    .bg(theme.secondary)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if selected {
                        theme.primary
                    } else {
                        theme.group_box_foreground
                    })
                    .child(format!("M{}", u16::from(index) + 1)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .py_1()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(NAME_TEXT))
                            .line_height(px(ROW_LINE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(SharedString::from(summary.name.as_str().to_owned())),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(DETAIL_TEXT))
                            .text_color(theme.muted_foreground)
                            .child(detail),
                    ),
            )
            .child(self.row_menu(index, full, cx))
            .on_click(cx.listener(move |this, _, window, cx| this.select(index, window, cx)))
    }

    fn row_menu(&self, index: u8, full: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let page = cx.entity().downgrade();
        let recording = self.recorder.is_some();
        let items = Command::ALL.map(|command| {
            let item = Item::new(command.label())
                .disabled(recording || (command == Command::Duplicate && full));
            if command == Command::Delete {
                item.danger(true).divider_before()
            } else {
                item
            }
        });
        div()
            .id(SharedString::from(format!("macro-menu-{index}")))
            .flex_none()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                Dropdown::new(SharedString::from(format!("macro-commands-{index}")))
                    .trigger(kebab)
                    .align_end()
                    .menu_width(px(MENU_WIDTH))
                    .items(items)
                    .on_select(move |position, window, cx| {
                        let Some(command) = Command::ALL.get(position).copied() else {
                            return;
                        };
                        page.update(cx, |page, cx| page.run(command, index, window, cx))
                            .ok();
                    }),
            )
    }

    fn run(&mut self, command: Command, index: u8, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::Rename => self.start_rename(index, window, cx),
            Command::Duplicate => self.duplicate(index, window, cx),
            Command::Delete => self.confirm_delete(index, window, cx),
        }
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.free_slot(window, cx) else {
            return;
        };
        self.store_new(index, edit::created(index), window, cx);
        self.start_rename(index, window, cx);
    }

    fn duplicate(&mut self, source: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.with_content(source, window, cx, |page, value, window, cx| {
            if let Some(index) = page.free_slot(window, cx) {
                page.store_new(index, edit::duplicated(&value), window, cx);
            }
        });
    }

    fn confirm_delete(&self, index: u8, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        let name = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.macros.name(index))
            .unwrap_or_default()
            .to_owned();
        panel::confirm(
            panel::Confirmation::new(
                format!("Delete “{name}”?"),
                "Keys bound to this macro do nothing until a new macro takes its slot.",
                "Delete",
            )
            .danger()
            .alert(),
            move |window, cx| {
                page.update(cx, |page, cx| page.delete(index, window, cx))
                    .ok();
            },
            window,
            cx,
        );
    }

    fn delete(&mut self, index: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.with_content(index, window, cx, move |page, value, window, cx| {
            let applied = page.store.update(cx, |store, cx| {
                store.set_macro(index, Some(&value), None, cx)
            });
            if page.selected == Some(index) {
                page.content = None;
                page.revision += 1;
            }
            cx.spawn_in(window, async move |this, cx| {
                applied.await.ok();
                this.update_in(cx, |page, window, cx| {
                    if page.selected == Some(index) {
                        page.selected = None;
                    }
                    page.load(window, cx);
                })
                .ok();
            })
            .detach();
            cx.notify();
        });
    }

    fn start_rename(&mut self, index: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.select(index, window, cx);
        self.name.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
    }

    fn free_slot(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<u8> {
        let free = self
            .store
            .read(cx)
            .keyboard()
            .and_then(|keyboard| keyboard.macros.free());
        if free.is_none() {
            window.push_notification(Notification::warning(NO_ROOM), cx);
        }
        free
    }

    fn store_new(&mut self, index: u8, value: Macro, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(window, cx);
        self.selected = Some(index);
        self.content = Some((index, value.clone()));
        self.revision += 1;
        self.chosen = None;
        self.field = None;
        self.target = KeyTarget::Append;
        let applied = self.store.update(cx, |store, cx| {
            store.set_macro(index, None, Some(value), cx)
        });
        Self::reload_on_failure(applied, window, cx);
        self.sync_name(window, cx);
        self.sync_picker(cx);
        cx.notify();
    }

    fn with_content(
        &mut self,
        index: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Macro, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if let Some((loaded, value)) = &self.content
            && *loaded == index
            && self.selected == Some(index)
        {
            let value = value.clone();
            then(self, value, window, cx);
            return;
        }
        let fetch = self
            .store
            .update(cx, |store, cx| store.fetch::<GetMacro, _>(index, cx));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Some(value)) = Box::pin(fetch).await {
                this.update_in(cx, |page, window, cx| then(page, value, window, cx))
                    .ok();
            }
        })
        .detach();
    }
}
