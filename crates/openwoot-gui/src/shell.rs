use std::collections::HashMap;

use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, BoxShadow, Context, Div, Entity, FocusHandle,
    FontWeight, Hsla, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName as Glyph,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, Theme, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
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
    dropdown::{Close, Dropdown, Item},
    page::{Group, Page, ProfilesPage, profile_avatar},
    panel::{self, NAVIGATION_WIDTH, RAIL_WIDTH},
    store::{Keyboard, Link, Store, StoreEvent},
};

const HEADER_HEIGHT: f32 = 64.0;
const PROFILE_MENU_WIDTH: f32 = 520.0;
const VERSION: &str = env!("CARGO_PKG_VERSION");
const KEY_CONTEXT: &str = "Shell";
const HELP_MENU_WIDTH: f32 = 220.0;
const PROFILE_BADGE_REMS: f32 = 2.25;
const DOCUMENTATION_URL: &str = "https://github.com/Mewski/openwoot/tree/main/docs";
const ISSUES_URL: &str = "https://github.com/Mewski/openwoot/issues";
const SOURCE_URL: &str = "https://github.com/Mewski/openwoot";

gpui_kit::actions!(openwoot, [Undo, Redo]);

pub(crate) fn bind_keys(cx: &mut App) {
    let (undo, redo) = if cfg!(target_os = "macos") {
        ("cmd-z", "cmd-shift-z")
    } else {
        ("ctrl-z", "ctrl-shift-z")
    };
    cx.bind_keys([
        KeyBinding::new(undo, Undo, Some(KEY_CONTEXT)),
        KeyBinding::new(redo, Redo, Some(KEY_CONTEXT)),
    ]);
}

pub(crate) struct Shell {
    store: Entity<Store>,
    page: Page,
    pages: HashMap<Page, AnyView>,
    connection: Entity<ConnectionPanel>,
    navigation_open: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct ProfileEntry {
    slot: Slot,
    name: SharedString,
    color: Hsla,
    icon: u8,
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
                        this.page = Page::Quick;
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
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let refocus = cx.on_focus_lost(window, |this: &mut Self, window, cx| {
            window.focus(&this.focus, cx);
        });
        Self {
            store,
            page: Page::Quick,
            pages: HashMap::new(),
            connection,
            navigation_open: true,
            focus,
            _subscriptions: vec![subscription, observation, refocus],
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

    fn open_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Page::Profiles, cx);
        if let Ok(profiles) = self.view(window, cx).downcast::<ProfilesPage>() {
            profiles.update(cx, |profiles, cx| profiles.open_creator(window, cx));
        }
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
            (Group::Keyboard, Page::Remap, "Keyboard", Page::Quick),
            (Group::Device, Page::Settings, "Settings", Page::Settings),
        ];
        v_flex()
            .flex_none()
            .w(px(RAIL_WIDTH))
            .h_full()
            .items_center()
            .gap_2()
            .pt_4()
            .pb_4()
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
            .when(!self.navigation_open, |this| {
                this.child(
                    Button::new("navigation-open")
                        .ghost()
                        .icon(IconName::PanelLeftOpen)
                        .tooltip("Show navigation")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigation_open = true;
                            cx.notify();
                        })),
                )
            })
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
            .child(div().flex_1())
            .child(help_menu(&cx.entity(), &theme))
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
                ("Profiles", vec![Page::Quick, Page::Profiles]),
                (
                    "Configuration",
                    keyboard_available
                        .iter()
                        .copied()
                        .filter(|page| page.in_configuration())
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
                h_flex()
                    .pl_1()
                    .pt_1()
                    .justify_between()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        Button::new("navigation-close")
                            .ghost()
                            .small()
                            .icon(IconName::PanelLeftClose)
                            .tooltip("Hide navigation")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigation_open = false;
                                cx.notify();
                            })),
                    ),
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
        let (can_undo, can_redo) = (store.can_undo(), store.can_redo());
        let profiles = profile_entries(keyboard);
        let active = keyboard.map(|keyboard| keyboard.active);
        let room = keyboard.is_some_and(|keyboard| {
            keyboard.profiles.len() < usize::from(keyboard.capabilities.profiles)
        });
        let current = profiles
            .iter()
            .find(|entry| Some(entry.slot) == active)
            .cloned();
        let shell = cx.entity();
        let switcher = current.map(|entry| {
            div().w(px(PROFILE_MENU_WIDTH)).child(
                Dropdown::new("profile-switcher")
                    .menu_width(px(PROFILE_MENU_WIDTH))
                    .trigger(move |open, cx| profile_trigger(&entry, open, cx))
                    .content(move |close, _, cx| {
                        profile_menu(&shell, &profiles, active, room, close, cx)
                    }),
            )
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
                        .child(
                            Button::new("undo")
                                .ghost()
                                .icon(IconName::Undo2)
                                .tooltip("Undo")
                                .disabled(!can_undo)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.store.update(cx, Store::undo);
                                })),
                        )
                        .child(
                            Button::new("redo")
                                .ghost()
                                .icon(IconName::Redo2)
                                .tooltip("Redo")
                                .disabled(!can_redo)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.store.update(cx, Store::redo);
                                })),
                        )
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
            .track_focus(&self.focus)
            .key_context(KEY_CONTEXT)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if window.focused(cx).is_none() {
                        window.focus(&this.focus, cx);
                    }
                }),
            )
            .on_action(cx.listener(|this, _: &Undo, _, cx| {
                this.store.update(cx, Store::undo);
            }))
            .on_action(cx.listener(|this, _: &Redo, _, cx| {
                this.store.update(cx, Store::redo);
            }))
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.rail(cx))
            .when(self.navigation_open, |this| this.child(self.navigation(cx)))
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

fn help_menu(shell: &Entity<Shell>, theme: &Theme) -> Dropdown {
    let muted = theme.muted_foreground;
    let hover = theme.secondary_hover;
    let radius = theme.radius;
    let shell = shell.clone();
    let links = [
        ("Documentation", Glyph::BookOpen, DOCUMENTATION_URL),
        ("Report a problem", Glyph::Bug, ISSUES_URL),
        ("Source code", Glyph::Github, SOURCE_URL),
    ];
    Dropdown::new("help")
        .menu_width(px(HELP_MENU_WIDTH))
        .trigger(move |open, cx| {
            v_flex()
                .w(px(RAIL_WIDTH - 12.0))
                .py_2()
                .gap_1()
                .items_center()
                .rounded(radius)
                .when(open, |this| this.bg(cx.theme().secondary))
                .hover(move |style| style.bg(hover))
                .child(
                    Icon::new(Glyph::CircleQuestionMark)
                        .small()
                        .text_color(muted),
                )
                .child(div().text_xs().text_color(muted).child("Help"))
                .into_any_element()
        })
        .items(
            std::iter::once(Item::new("Troubleshoot").icon(Glyph::Wrench)).chain(
                links
                    .iter()
                    .map(|(label, icon, _)| Item::new(*label).icon(*icon)),
            ),
        )
        .on_select(move |index, _, cx| match index.checked_sub(1) {
            None => shell.update(cx, |shell, cx| shell.show(Page::Firmware, cx)),
            Some(link) => {
                if let Some((_, _, url)) = links.get(link) {
                    cx.open_url(url);
                }
            }
        })
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

fn profile_entries(keyboard: Option<&Keyboard>) -> Vec<ProfileEntry> {
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
                Some(ProfileEntry {
                    slot,
                    name: info.name.as_str().to_owned().into(),
                    color: color.into(),
                    icon: info.icon,
                })
            })
            .collect()
    })
}

fn profile_badge(entry: &ProfileEntry, cx: &App) -> Div {
    let size = cx.theme().font_size * PROFILE_BADGE_REMS;
    profile_avatar(entry.color, entry.icon, size, cx).shadow(vec![BoxShadow {
        color: entry.color.opacity(0.55),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(14.0),
        spread_radius: px(1.0),
        inset: false,
    }])
}

fn profile_trigger(entry: &ProfileEntry, open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.accent;
    h_flex()
        .h(px(52.0))
        .w_full()
        .px_2()
        .gap_3()
        .rounded(theme.radius)
        .when(open, |this| this.bg(theme.accent))
        .hover(move |style| style.bg(hover))
        .child(profile_badge(entry, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(entry.name.clone()),
        )
        .child(
            Icon::new(if open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .small()
            .text_color(theme.muted_foreground),
        )
        .into_any_element()
}

fn profile_menu(
    shell: &Entity<Shell>,
    profiles: &[ProfileEntry],
    active: Option<Slot>,
    room: bool,
    close: &Close,
    cx: &App,
) -> AnyElement {
    let created = (shell.clone(), close.clone());
    let managed = (shell.clone(), close.clone());
    let cards = profiles
        .iter()
        .map(|entry| profile_card(shell, close, entry, Some(entry.slot) == active, cx));
    v_flex()
        .p_3()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child(div().flex_1().child(panel::label("Onboard profiles", cx)))
                .child(
                    Button::new("profile-menu-manage")
                        .ghost()
                        .small()
                        .label("Manage")
                        .on_click(move |_, _, cx| {
                            let (shell, close) = &managed;
                            close.close(cx);
                            shell.update(cx, |shell, cx| shell.show(Page::Profiles, cx));
                        }),
                )
                .child(
                    Button::new("profile-menu-new")
                        .small()
                        .icon(IconName::Plus)
                        .label("New profile")
                        .disabled(!room)
                        .on_click(move |_, window, cx| {
                            let (shell, close) = &created;
                            close.close(cx);
                            shell.update(cx, |shell, cx| shell.open_creator(window, cx));
                        }),
                ),
        )
        .children(cards)
        .into_any_element()
}

fn profile_card(
    shell: &Entity<Shell>,
    close: &Close,
    entry: &ProfileEntry,
    current: bool,
    cx: &App,
) -> Stateful<Div> {
    let slot = entry.slot;
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    let picked = (shell.clone(), close.clone());
    h_flex()
        .id(SharedString::from(format!("profile-card-{}", slot.get())))
        .h(px(64.0))
        .px_3()
        .gap_3()
        .rounded(theme.radius)
        .bg(theme.secondary)
        .border_1()
        .border_color(if current {
            theme.primary
        } else {
            theme.secondary
        })
        .cursor_pointer()
        .when(!current, |this| this.hover(move |style| style.bg(hover)))
        .on_click(move |_, _, cx| {
            let (shell, close) = &picked;
            close.close(cx);
            if !current {
                shell.update(cx, |shell, cx| {
                    shell
                        .store
                        .update(cx, |store, cx| store.activate(slot, cx))
                        .detach();
                });
            }
        })
        .child(
            div()
                .size_5()
                .flex_none()
                .rounded(px(4.0))
                .bg(theme.background)
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(SharedString::from(
                    (usize::from(slot.get()) + 1).to_string(),
                )),
        )
        .child(profile_badge(entry, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .child(entry.name.clone()),
        )
        .when(current, |this| {
            this.child(
                div()
                    .px_1p5()
                    .rounded(px(4.0))
                    .bg(theme.primary)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.primary_foreground)
                    .child("ACTIVE"),
            )
        })
}
