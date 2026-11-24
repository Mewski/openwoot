use std::collections::HashMap;

use gpui_kit::{
    AnyView, AppContext as _, BoxShadow, Context, Div, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Theme, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        menu::{DropdownMenu as _, PopupMenuItem},
        notification::Notification,
        spinner::Spinner,
        v_flex,
    },
    div, point,
    prelude::FluentBuilder as _,
    px, rgb,
};
use openwoot_protocol::profile::Slot;

use crate::{
    connection::ConnectionPanel,
    page::{Group, Page},
    panel::{NAVIGATION_WIDTH, RAIL_WIDTH},
    store::{Keyboard, Link, Store, StoreEvent},
};

const HEADER_HEIGHT: f32 = 64.0;
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct Shell {
    store: Entity<Store>,
    page: Page,
    pages: HashMap<Page, AnyView>,
    connection: Entity<ConnectionPanel>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe_in(&store, window, |this, _, event, window, cx| {
            match event {
                StoreEvent::Failed(message) => {
                    window.push_notification(Notification::error(message.clone()), cx);
                }
                StoreEvent::Disconnected => {
                    window.push_notification(Notification::warning("Keyboard disconnected"), cx);
                }
                StoreEvent::Connected => {
                    if !this.page.is_available(this.store.read(cx).keyboard()) {
                        this.page = Page::Performance;
                    }
                }
                StoreEvent::ProfileChanged | StoreEvent::Reloaded => {}
            }
            cx.notify();
        });
        let observation = cx.observe(&store, |_, _, cx| cx.notify());
        let shell = cx.entity().downgrade();
        let connection = cx.new(|cx| {
            ConnectionPanel::new(
                store.clone(),
                move |window, cx| {
                    shell
                        .update(cx, |shell, cx| shell.show(Page::Firmware, cx))
                        .ok();
                    window.close_dialog(cx);
                },
                cx,
            )
        });
        Self {
            store,
            page: Page::Performance,
            pages: HashMap::new(),
            connection,
            _subscriptions: vec![subscription, observation],
        }
    }

    fn open_connection(&self, window: &mut Window, cx: &mut Context<Self>) {
        let connected = self.store.read(cx).keyboard().is_some();
        if !connected {
            self.store.update(cx, Store::search);
        }
        let panel = self.connection.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(if connected { "Keyboard" } else { "Connection" })
                .w(px(480.0))
                .child(panel.clone())
        });
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let store = self.store.clone();
        self.pages
            .entry(self.page)
            .or_insert_with(|| self.page.open(&store, window, cx))
            .clone()
    }

    fn rail(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let current = self.page.group();
        let entries = [
            (Group::Keyboard, Page::Remap, "Keyboard", Page::Performance),
            (Group::Device, Page::Settings, "Settings", Page::Settings),
        ];
        v_flex()
            .flex_none()
            .w(px(RAIL_WIDTH))
            .h_full()
            .items_center()
            .gap_2()
            .pt_4()
            .bg(theme.sidebar)
            .child(
                div()
                    .mb_3()
                    .size_10()
                    .rounded(theme.radius)
                    .border_2()
                    .border_color(theme.foreground)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(div().text_sm().font_weight(FontWeight::BOLD).child("ow")),
            )
            .children(
                entries
                    .into_iter()
                    .map(|(group, icon_page, label, landing)| {
                        let selected = group == current;
                        v_flex()
                            .id(SharedString::from(format!("rail-{label}")))
                            .w(px(RAIL_WIDTH - 12.0))
                            .py_2()
                            .gap_1()
                            .items_center()
                            .rounded(theme.radius)
                            .cursor_pointer()
                            .when(selected, |this| this.bg(theme.secondary))
                            .hover(|style| style.bg(theme.secondary_hover))
                            .child(icon_page.icon().small().text_color(if selected {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if selected {
                                        theme.foreground
                                    } else {
                                        theme.muted_foreground
                                    })
                                    .child(label),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.page.group() != group {
                                    this.show(landing, cx);
                                }
                            }))
                    }),
            )
    }

    fn navigation(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let keyboard_available: Vec<Page> = Page::ALL
            .into_iter()
            .filter(|page| page.is_available(self.store.read(cx).keyboard()))
            .collect();
        let group = self.page.group();
        let current = self.page;
        let sections: Vec<(&str, Vec<Page>)> = match group {
            Group::Keyboard => vec![
                ("Profiles", vec![Page::Profiles]),
                (
                    "Configuration",
                    keyboard_available
                        .iter()
                        .copied()
                        .filter(|page| page.group() == Group::Keyboard && *page != Page::Profiles)
                        .collect(),
                ),
            ],
            Group::Device => vec![(
                "Device",
                keyboard_available
                    .iter()
                    .copied()
                    .filter(|page| page.group() == Group::Device)
                    .collect(),
            )],
        };
        let title = match group {
            Group::Keyboard => "Keyboard configuration",
            Group::Device => "Settings",
        };
        let device = self.device(cx);
        v_flex()
            .flex_none()
            .w(px(NAVIGATION_WIDTH))
            .my_4()
            .ml_4()
            .p_3()
            .gap_4()
            .rounded(theme.radius_lg)
            .bg(theme.group_box)
            .child(
                div()
                    .px_1()
                    .pt_1()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(device)
            .child(
                v_flex()
                    .id("navigation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap_4()
                    .children(sections.into_iter().map(|(label, pages)| {
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .px_3()
                                    .pb_1()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(label),
                            )
                            .children(pages.into_iter().map(|page| {
                                let selected = current == page;
                                nav_item(page, selected, &theme).on_click(
                                    cx.listener(move |this, _, _, cx| this.show(page, cx)),
                                )
                            }))
                    })),
            )
            .child(
                div()
                    .text_center()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("openwoot {VERSION}")),
            )
    }

    fn device(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let (caption, name, dot): (SharedString, SharedString, Hsla) =
            match (store.keyboard(), store.link()) {
                (Some(keyboard), _) => (
                    format!("Firmware {}", keyboard.version.firmware).into(),
                    keyboard.name.clone(),
                    theme.success,
                ),
                (None, Link::Loading) => ("Connecting".into(), "Keyboard".into(), theme.warning),
                (None, Link::Unreachable(_)) => {
                    ("Unavailable".into(), "Keyboard".into(), theme.danger)
                }
                (None, Link::Searching | Link::Connected) => (
                    "Not connected".into(),
                    "No keyboard".into(),
                    theme.muted_foreground,
                ),
            };
        h_flex().child(
            h_flex()
                .id("device")
                .w_full()
                .gap_3()
                .p_2()
                .rounded(theme.radius)
                .bg(theme.tab_active)
                .cursor_pointer()
                .hover(|style| style.bg(theme.secondary_hover))
                .on_click(cx.listener(|this, _, window, cx| this.open_connection(window, cx)))
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .size_12()
                        .rounded(theme.radius)
                        .bg(theme.sidebar)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Page::Remap
                                .icon()
                                .small()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(px(3.0))
                                .right(px(3.0))
                                .size(px(8.0))
                                .rounded_full()
                                .bg(dot),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(caption),
                        )
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(name),
                        ),
                )
                .child(
                    Icon::new(IconName::ChevronDown)
                        .small()
                        .text_color(theme.foreground),
                ),
        )
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let keyboard = store.keyboard();
        let unsaved = keyboard.is_some_and(|keyboard| keyboard.unsaved);
        let profiles = profile_entries(keyboard);
        let active = keyboard.map(|keyboard| keyboard.active);
        let current = profiles
            .iter()
            .find(|(slot, _, _)| Some(*slot) == active)
            .cloned();
        let shell = cx.entity();
        let switcher = current.map(|(_, name, color)| {
            Button::new("profile-switcher")
                .ghost()
                .h(px(48.0))
                .px_2()
                .child(
                    h_flex()
                        .gap_3()
                        .child(profile_badge(color, &theme))
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            Icon::new(IconName::ChevronDown)
                                .small()
                                .text_color(theme.muted_foreground),
                        ),
                )
                .dropdown_menu(move |mut menu, window, _| {
                    for (slot, name, _) in &profiles {
                        let slot = *slot;
                        menu = menu.item(
                            PopupMenuItem::new(name.clone())
                                .checked(Some(slot) == active)
                                .on_click(window.listener_for(&shell, move |this, _, _, cx| {
                                    if Some(slot) != active {
                                        this.store
                                            .update(cx, |store, cx| store.activate(slot, cx))
                                            .detach();
                                    }
                                })),
                        );
                    }
                    menu.separator()
                        .item(
                            PopupMenuItem::new("Manage profiles").on_click(window.listener_for(
                                &shell,
                                |this, _, _, cx| {
                                    this.show(Page::Profiles, cx);
                                },
                            )),
                        )
                })
        });
        h_flex()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .mx_4()
            .mt_4()
            .px_3()
            .gap_2()
            .justify_between()
            .rounded(theme.radius_lg)
            .bg(theme.group_box)
            .child(h_flex().children(switcher))
            .when(keyboard.is_some(), |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .when(unsaved, |this| {
                            this.child(Button::new("revert").ghost().label("Discard").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.store.update(cx, |store, cx| store.revert(cx)).detach();
                                }),
                            ))
                        })
                        .child(
                            Button::new("save")
                                .primary()
                                .label("Save")
                                .disabled(!unsaved)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.store.update(cx, |store, cx| store.save(cx)).detach();
                                })),
                        ),
                )
            })
    }

    fn waiting(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let (title, detail): (&str, SharedString) = match store.link() {
            Link::Loading => ("Connecting", SharedString::default()),
            Link::Unreachable(reason) => ("Keyboard unavailable", reason.clone()),
            Link::Searching | Link::Connected => (
                "No keyboard connected",
                "Plug in a keyboard running openwoot.".into(),
            ),
        };
        let loading = matches!(store.link(), Link::Loading);
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .when(loading, |this| this.child(Spinner::new()))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .when(!detail.is_empty(), |this| {
                this.child(
                    div()
                        .max_w(px(420.0))
                        .text_center()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(detail),
                )
            })
            .when(!loading, |this| {
                this.child(
                    h_flex()
                        .mt_4()
                        .gap_2()
                        .child(Button::new("waiting-help").label("Troubleshoot").on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_connection(window, cx);
                            }),
                        ))
                        .child(
                            Button::new("waiting-firmware")
                                .primary()
                                .label("Install firmware")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show(Page::Firmware, cx);
                                })),
                        ),
                )
            })
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let connected = self.store.read(cx).keyboard().is_some();
        let content = if connected || !self.page.needs_keyboard() {
            div()
                .id("page")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(self.view(window, cx))
                .into_any_element()
        } else {
            self.waiting(cx).into_any_element()
        };
        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.rail(cx))
            .child(self.navigation(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.header(cx))
                    .child(content),
            )
    }
}

fn nav_item(page: Page, selected: bool, theme: &Theme) -> Stateful<Div> {
    let hover = theme.secondary;
    h_flex()
        .id(SharedString::from(format!("page-{}", page.title())))
        .h_10()
        .px_3()
        .gap_3()
        .rounded(theme.radius)
        .text_sm()
        .cursor_pointer()
        .text_color(if selected {
            theme.foreground
        } else {
            theme.sidebar_foreground
        })
        .when(selected, |this| this.bg(theme.secondary_hover))
        .hover(move |style| style.bg(hover))
        .child(page.icon().small().text_color(if selected {
            theme.primary
        } else {
            theme.muted_foreground
        }))
        .child(page.title())
}

fn profile_entries(keyboard: Option<&Keyboard>) -> Vec<(Slot, SharedString, Hsla)> {
    keyboard.map_or_else(Vec::new, |keyboard| {
        keyboard
            .profiles
            .iter()
            .enumerate()
            .filter_map(|(index, info)| {
                let slot = Slot::new(u8::try_from(index).ok()?);
                let color = rgb(u32::from_be_bytes([
                    0,
                    info.color.red,
                    info.color.green,
                    info.color.blue,
                ]));
                Some((slot, info.name.as_str().to_owned().into(), color.into()))
            })
            .collect()
    })
}

fn profile_badge(color: Hsla, theme: &Theme) -> Div {
    div()
        .size_9()
        .rounded_full()
        .border_2()
        .border_color(color)
        .bg(theme.sidebar)
        .shadow(vec![BoxShadow {
            color: color.opacity(0.55),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(14.0),
            spread_radius: px(1.0),
            inset: false,
        }])
        .flex()
        .items_center()
        .justify_center()
        .child(div().size_3().rounded_full().bg(color))
}
