mod restore;
mod update;

use std::path::{Path, PathBuf};

use gpui_kit::{
    AnyElement, AppContext as _, Context, Div, Entity, ExternalPaths, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        spinner::Spinner,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_host::flash::{self, Plan};

use self::update::{Update, error_chain};
use super::{
    Page,
    diagnostics::open_device_info,
    settings::{Tone, callout, entry, layout_name, sheet},
};
use crate::{
    dropdown::{Dropdown, Item},
    panel,
    store::{Keyboard, Link, Store},
};

const UPDATE_DIALOG_WIDTH: f32 = 520.0;
const MENU_WIDTH: f32 = 200.0;
const MENU_BUTTON: f32 = 32.0;
const STATUS_DOT: f32 = 8.0;

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
            Ok(Err(error)) => {
                this.update(cx, |page, cx| {
                    page.check = Some(Check::Rejected(
                        format!("could not open the file dialog: {error:#}").into(),
                    ));
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
                        error_chain(&flash::Error::NoUpdateMode { board: plan.board }).into(),
                    )),
                    Err(error) => Some(Check::Rejected(error_chain(&error).into())),
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
                .close_button(closable)
                .overlay_closable(closable)
                .keyboard(closable)
                .child(update.clone())
        });
    }

    fn device_section(&self, cx: &Context<Self>) -> Div {
        let store = self.store.read(cx);
        let section = entry(
            "My keyboard",
            Some("The keyboard connected to openwoot and the firmware it runs.".into()),
            None,
            cx,
        );
        match (store.keyboard(), store.link()) {
            (Some(keyboard), _) => section.child(device_card(keyboard, &self.store, cx)),
            (None, Link::Unreachable(reason)) => section
                .child(panel::label("No keyboard", cx))
                .child(panel::note(reason.clone(), cx)),
            (None, Link::Searching | Link::Loading | Link::Connected) => {
                section.child(panel::label("No keyboard", cx))
            }
        }
    }

    fn file_section(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let accent = theme.primary;
        let raised = theme.tab_active;
        let status = match &self.check {
            None => None,
            Some(Check::Checking(file)) => Some(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(Spinner::new().small())
                    .child(format!("Checking {file}"))
                    .into_any_element(),
            ),
            Some(Check::Rejected(reason)) => {
                Some(callout(Tone::Danger, reason.clone(), cx).into_any_element())
            }
        };
        entry(
            "Install firmware from a file",
            Some(
                "Checks an openwoot image (.hex or .bin) and installs it through the factory \
                 bootloader."
                    .into(),
            ),
            None,
            cx,
        )
        .child(
            v_flex()
                .id("firmware-drop")
                .w_full()
                .items_center()
                .gap_3()
                .py_8()
                .rounded(theme.radius_lg)
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .bg(theme.group_box)
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.border_color(accent).bg(raised)
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                    this.drop_files(paths, window, cx);
                }))
                .child(
                    Icon::new(Catalog::FileUp)
                        .large()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Drop a firmware file here"),
                )
                .child(
                    Button::new("choose-file")
                        .primary()
                        .label("Choose file")
                        .disabled(self.checking())
                        .on_click(cx.listener(|_, _, window, cx| Self::choose_file(window, cx))),
                ),
        )
        .children(status)
    }
}

impl Render for FirmwarePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sections = vec![
            self.device_section(cx),
            self.file_section(cx),
            restore::section(cx),
        ];
        panel::page().child(sheet(Page::Firmware.title(), sections, cx))
    }
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
    h_flex()
        .w_full()
        .gap_4()
        .p_3()
        .rounded(theme.radius_lg)
        .bg(theme.group_box)
        .child(
            div()
                .relative()
                .flex_none()
                .size_16()
                .rounded(theme.radius)
                .bg(theme.sidebar)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(Catalog::Keyboard)
                        .large()
                        .text_color(theme.muted_foreground),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .left(px(6.0))
                        .size(px(STATUS_DOT))
                        .rounded_full()
                        .bg(theme.success),
                ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.foreground)
                                .child(keyboard.name.clone()),
                        )
                        .child(Tag::success().small().child("CONNECTED")),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x_3()
                        .text_sm()
                        .children(details.into_iter().map(|(label, value)| {
                            h_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(theme.muted_foreground)
                                        .child(format!("{label}:")),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(value),
                                )
                        })),
                ),
        )
        .child(
            div().flex_none().w(px(MENU_BUTTON)).child(
                Dropdown::new("device-menu")
                    .menu_width(px(MENU_WIDTH))
                    .align_end()
                    .trigger(menu_button)
                    .items([Item::new("View device info").icon(Catalog::Info)])
                    .on_select(move |_, window, cx| {
                        let keyboard = store.read(cx).keyboard().cloned();
                        if let Some(keyboard) = keyboard {
                            open_device_info(&keyboard, window, cx);
                        }
                    }),
            ),
        )
}

fn menu_button(open: bool, cx: &gpui_kit::App) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.secondary;
    div()
        .size(px(MENU_BUTTON))
        .rounded(theme.radius)
        .flex()
        .items_center()
        .justify_center()
        .text_color(theme.muted_foreground)
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
