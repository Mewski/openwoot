mod device_info;
mod rename;
mod restore;
mod update;

use std::path::{Path, PathBuf};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, ExternalPaths, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, spinner::Spinner, tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_host::flash::{self, Plan};

use self::{
    device_info::open_device_info,
    rename::Rename,
    restore::restore_section,
    update::{Update, flash_text},
};
use super::{
    Page,
    settings::{Tone, body_text, callout, heading, layout_name, sheet},
};
use crate::{
    dropdown::{Dropdown, Item},
    panel::ActionButton,
    store::{Keyboard, Link, Store},
};

const UPDATE_DIALOG_WIDTH: f32 = 520.0;
const MENU_WIDTH: f32 = 200.0;
const RENAME_ITEM: usize = 1;
const MENU_BUTTON: f32 = 32.0;
const HEADING_GAP: f32 = 16.0;
const DESCRIPTION_GAP: f32 = 24.0;
const CARD_RADIUS: f32 = 6.0;
const CARD_PADDING: f32 = 12.0;
const CARD_GAP: f32 = 12.0;
const IMAGE_WIDTH: f32 = 100.0;
const IMAGE_HEIGHT: f32 = 83.0;
const STATUS_DOT: f32 = 8.0;
const STATUS_INSET: f32 = 4.0;
const NAME_SIZE: f32 = 16.0;
const NAME_LINE: f32 = 22.0;
const NAME_GAP: f32 = 8.0;
const INFO_SIZE: f32 = 14.0;
const INFO_LINE: f32 = 18.0;
const EMPTY_SIZE: f32 = 14.0;
const EMPTY_LINE: f32 = 16.8;
const DROP_PADDING: f32 = 32.0;
const DROP_GAP: f32 = 12.0;
const STATUS_GAP: f32 = 16.0;

pub(crate) struct FirmwarePage {
    store: Entity<Store>,
    check: Option<Check>,
    _subscriptions: Vec<Subscription>,
}

enum Check {
    Checking(SharedString),
    Rejected(SharedString),
}

impl FirmwarePage {
    pub(crate) fn new(store: Entity<Store>, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            store,
            check: None,
            _subscriptions: vec![observation],
        }
    }

    fn checking(&self) -> bool {
        matches!(self.check, Some(Check::Checking(_)))
    }

    fn choose_file(window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose firmware file".into()),
        });
        cx.spawn_in(window, async move |this, cx| match chosen.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    this.update_in(cx, |page, window, cx| page.check_file(path, window, cx))
                        .ok();
                }
            }
            Ok(Err(_)) => {
                this.update(cx, |page, cx| {
                    page.check = Some(Check::Rejected("Could not open the file dialog.".into()));
                    cx.notify();
                })
                .ok();
            }
            Ok(Ok(None)) | Err(_) => {}
        })
        .detach();
    }

    fn drop_files(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = paths.paths().first() {
            self.check_file(path.clone(), window, cx);
        }
    }

    fn check_file(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.checking() {
            return;
        }
        self.check = Some(Check::Checking(file_name(&path)));
        cx.notify();
        let planned = cx
            .background_executor()
            .spawn(async move { flash::plan(&path) });
        cx.spawn_in(window, async move |this, cx| {
            let planned = planned.await;
            this.update_in(cx, |page, window, cx| {
                page.check = match planned {
                    Ok(plan) if plan.update.is_some() => {
                        page.open_update(plan, window, cx);
                        None
                    }
                    Ok(plan) => Some(Check::Rejected(
                        flash_text(&flash::Error::NoUpdateMode { board: plan.board }).into(),
                    )),
                    Err(error) => Some(Check::Rejected(flash_text(&error).into())),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_update(&self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let update = cx.new(|cx| Update::new(store, plan, cx));
        window.open_dialog(cx, move |dialog, _, cx| {
            let closable = update.read(cx).closable();
            dialog
                .w(px(UPDATE_DIALOG_WIDTH))
                .centered()
                .close_button(closable)
                .overlay_closable(closable)
                .keyboard(closable)
                .child(update.clone())
        });
    }

    fn device_section(&self, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let section = section(
            "My devices",
            "Manage your Wooting devices and keep them up to date.",
            cx,
        );
        match (store.keyboard(), store.link()) {
            (Some(keyboard), _) => section.child(device_card(keyboard, &self.store, cx)),
            (None, Link::Unreachable(reason)) => section
                .child(no_devices(cx))
                .child(body_text(reason.clone(), cx).mt(px(HEADING_GAP))),
            (None, Link::Searching | Link::Loading | Link::Connected) => {
                section.child(no_devices(cx))
            }
        }
    }

    fn file_section(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let ring = theme.primary;
        let raised = theme.secondary;
        let status = match &self.check {
            None => None,
            Some(Check::Checking(file)) => Some(
                h_flex()
                    .gap_2()
                    .text_size(px(INFO_SIZE))
                    .text_color(theme.group_box_foreground)
                    .child(Spinner::new().small())
                    .child(format!("Checking {file}"))
                    .into_any_element(),
            ),
            Some(Check::Rejected(reason)) => {
                Some(callout(Tone::Danger, reason.clone(), cx).into_any_element())
            }
        };
        section(
            "Install firmware from a file",
            "Installs an openwoot image (.hex or .bin) through the factory bootloader.",
            cx,
        )
        .child(
            v_flex()
                .id("firmware-drop")
                .w_full()
                .items_center()
                .gap(px(DROP_GAP))
                .py(px(DROP_PADDING))
                .rounded(theme.radius)
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .bg(theme.accent)
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.border_color(ring).bg(raised)
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    this.drop_files(paths, window, cx);
                }))
                .child(
                    Icon::new(Catalog::FileUp)
                        .large()
                        .text_color(theme.sidebar_foreground),
                )
                .child(
                    div()
                        .text_size(px(INFO_SIZE))
                        .text_color(theme.group_box_foreground)
                        .child("Drop a firmware file here"),
                )
                .child(
                    ActionButton::new("choose-file", "Choose file")
                        .primary()
                        .disabled(self.checking())
                        .on_click(cx.listener(|_, _, window, cx| Self::choose_file(window, cx))),
                ),
        )
        .when_some(status, |this, status| {
            this.child(div().w_full().mt(px(STATUS_GAP)).child(status))
        })
    }
}

impl Render for FirmwarePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = vec![
            self.device_section(cx),
            self.file_section(cx),
            restore_section(
                self.checking(),
                cx.listener(|_, _, window, cx| Self::choose_file(window, cx)),
                cx,
            ),
        ];
        sheet("firmware-sheet", Page::Firmware.title(), sections, cx)
    }
}

fn section(title: &'static str, description: &'static str, cx: &App) -> Div {
    v_flex()
        .w_full()
        .child(heading(title, None, cx).mb(px(HEADING_GAP)))
        .child(body_text(description, cx).mb(px(DESCRIPTION_GAP)))
}

fn no_devices(cx: &App) -> Div {
    div()
        .text_size(px(EMPTY_SIZE))
        .line_height(px(EMPTY_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(cx.theme().sidebar_foreground)
        .child("NO DEVICES")
}

fn device_card(keyboard: &Keyboard, store: &Entity<Store>, cx: &Context<FirmwarePage>) -> Div {
    let theme = cx.theme();
    let details = [
        ("Firmware", keyboard.version.firmware.to_string()),
        ("Protocol", keyboard.version.protocol.to_string()),
        (
            "Layout",
            keyboard.layout.map_or("Unknown", layout_name).to_owned(),
        ),
    ];
    let store = store.clone();
    let hover = theme.secondary;
    h_flex()
        .w_full()
        .items_start()
        .gap(px(CARD_GAP))
        .p(px(CARD_PADDING))
        .rounded(px(CARD_RADIUS))
        .bg(theme.accent)
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(IMAGE_WIDTH))
                .h(px(IMAGE_HEIGHT))
                .rounded(theme.radius)
                .bg(theme.sidebar)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(Catalog::Keyboard)
                        .large()
                        .text_color(theme.sidebar_foreground),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(STATUS_INSET))
                        .left(px(STATUS_INSET))
                        .size(px(STATUS_DOT))
                        .rounded_full()
                        .bg(theme.success),
                ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .gap(px(NAME_GAP))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(NAME_SIZE))
                                .line_height(px(NAME_LINE))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(keyboard.display_name()),
                        )
                        .child(Tag::success().small().child("CONNECTED")),
                )
                .children(details.into_iter().map(|(label, value)| {
                    h_flex()
                        .gap_1()
                        .text_size(px(INFO_SIZE))
                        .line_height(px(INFO_LINE))
                        .text_color(theme.sidebar_foreground)
                        .child(format!("{label}:"))
                        .child(div().font_weight(FontWeight::BOLD).child(value))
                })),
        )
        .child(
            div().flex_none().w(px(MENU_BUTTON)).child(
                Dropdown::new("device-menu")
                    .menu_width(px(MENU_WIDTH))
                    .align_end()
                    .trigger(menu_button)
                    .items([Item::new("View device info"), Item::new("Rename device")])
                    .on_select(move |index, window, cx| {
                        if index == RENAME_ITEM {
                            Rename::open(&store, window, cx);
                        } else if let Some(keyboard) = store.read(cx).keyboard().cloned() {
                            open_device_info(&keyboard, window, cx);
                        }
                    }),
            ),
        )
}

fn menu_button(open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.secondary;
    div()
        .size(px(MENU_BUTTON))
        .rounded(px(CARD_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.sidebar_foreground)
        .when(open, |this| this.bg(theme.secondary))
        .hover(move |style| style.bg(hover))
        .child(Icon::new(Catalog::Ellipsis).small())
        .into_any_element()
}

fn file_name(path: &Path) -> SharedString {
    path.file_name()
        .map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
        .into()
}
