use std::collections::HashMap;

use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, BoxShadow, Context, Div, Entity, FocusHandle,
    FontWeight, Hsla, InteractiveElement as _, IntoElement, KeyBinding, MouseButton,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName as Glyph,
    component::{
        ActiveTheme as _, Icon, IconName, Sizable as _, Theme, WindowExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        notification::Notification,
        spinner::Spinner,
        v_flex,
    },
    div, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder as _,
    px, rgb,
};
use openwoot_protocol::{
    keys::{Action, Function},
    profile::Slot,
};

use crate::{
    connection::ConnectionPanel,
    dropdown::{Close, Dropdown, Item},
    page::{Group, Page, ProfilesPage, profile_avatar},
    panel::{
        self, ActionButton, Frame, HEADER_HEIGHT, NAVIGATION_WIDTH, RAIL_WIDTH, SHELL_GAP, Tip,
    },
    store::{Keyboard, Link, Store, StoreEvent},
    theme, usage,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const KEY_CONTEXT: &str = "Shell";
const DOCUMENTATION_URL: &str = "https://github.com/Mewski/openwoot/tree/main/docs";
const ISSUES_URL: &str = "https://github.com/Mewski/openwoot/issues";
const SOURCE_URL: &str = "https://github.com/Mewski/openwoot";
const SHORTCUTS_TITLE: &str = "Onboard Profile Shortcuts";
const SHORTCUTS_HELP: &str = "Use this shortcut to quickly swap to this profile.";
const UNDO_HELP: &str = "to Undo";
const REDO_HELP: &str = "to Redo";
const USAGE_Z: u8 = 0x1D;
const CONFIGURATION: [Page; 7] = [
    Page::Actuation,
    Page::RapidTrigger,
    Page::Lighting,
    Page::Lightbar,
    Page::Remap,
    Page::Advanced,
    Page::Gamepad,
];
const RADIUS: f32 = 8.0;
const SMALL_RADIUS: f32 = 6.0;
const ICON: f32 = 20.0;
const RAIL_PADDING_Y: f32 = 20.0;
const RAIL_PADDING_X: f32 = 8.0;
const RAIL_LOGO_HEIGHT: f32 = 28.0;
const RAIL_LOGO: f32 = 40.0;
const RAIL_LOGO_GAP: f32 = 24.0;
const RAIL_ITEM_GAP: f32 = 16.0;
const RAIL_ITEM_HEIGHT: f32 = 48.0;
const RAIL_LABEL_LINE: f32 = 16.0;
const RAIL_ITEM_INNER_GAP: f32 = 4.0;
const RAIL_LABEL_TEXT: f32 = 12.0;
const NAVIGATION_PADDING: f32 = 16.0;
const NAVIGATION_GAP: f32 = 16.0;
const NAVIGATION_TITLE_HEIGHT: f32 = 32.0;
const BODY_TEXT: f32 = 16.0;
const BODY_LINE: f32 = 19.2;
const SECTION_GAP: f32 = 8.0;
const SECTION_LABEL_TEXT: f32 = 12.0;
const SECTION_LABEL_LINE: f32 = 14.4;
const SECTION_LABEL_PADDING_X: f32 = 16.0;
const SECTION_LABEL_PADDING_Y: f32 = 8.0;
const NAVIGATION_ITEM_HEIGHT: f32 = 40.0;
const NAVIGATION_ITEM_PADDING: f32 = 16.0;
const NAVIGATION_ITEM_GAP: f32 = 8.0;
const VERSION_LINE: f32 = 24.0;
const DEVICE_PADDING: f32 = 8.0;
const DEVICE_IMAGE: f32 = 58.0;
const COLLAPSED_WIDTH: f32 = 90.0;
const COLLAPSED_PADDING_X: f32 = 12.0;
const COLLAPSED_DEVICE_PADDING: f32 = 4.0;
const FOCUS_DIM: f32 = 0.384;
const DEVICE_GAP: f32 = 8.0;
const DEVICE_TEXT_GAP: f32 = 4.0;
const STATUS_DOT: f32 = 8.0;
const STATUS_INSET: f32 = 4.0;
const HEADER_PADDING: f32 = 20.0;
const HEADER_GAP: f32 = 16.0;
const ACTIONS_HEIGHT: f32 = 72.0;
const ACTIONS_TOP: f32 = 32.0;
const ACTIONS_RIGHT: f32 = 32.0;
const SAVE_PADDING: f32 = 12.0;
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_OPACITY: f32 = 0.6;
const TRIGGER_GAP: f32 = 12.0;
const AVATAR: f32 = 48.0;
const AVATAR_GLOW: f32 = 16.0;
const GLOW_WIDTH: f32 = 150.0;
const GLOW_ALPHA: f32 = 0.15;
const GLOW_START: f32 = 0.25;
const MENU_PADDING: f32 = 16.0;
const MENU_TOP_GAP: f32 = 20.0;
const MENU_GAP: f32 = 8.0;
const MENU_LIST_RIGHT: f32 = 6.4;
const MENU_LIST_BOTTOM: f32 = 20.0;
const SEARCH_HEIGHT: f32 = 40.0;
const SEARCH_ICON_INSET: f32 = 6.0;
const SEARCH_TEXT_INSET: f32 = 32.0;
const CARD_HEIGHT: f32 = 72.0;
const CARD_PADDING_X: f32 = 20.0;
const CARD_PADDING_Y: f32 = 12.0;
const CARD_GAP: f32 = 8.0;
const SLOT_SIZE: f32 = 20.0;
const SLOT_INSET: f32 = 3.0;
const SLOT_TEXT: f32 = 12.0;
const SLOT_LINE: f32 = 18.0;
const OUTLINE: f32 = 1.0;
const TAG_HEIGHT: f32 = 20.0;
const TAG_PADDING: f32 = 4.0;
const TAG_TEXT: f32 = 12.0;
const HELP_MENU_WIDTH: f32 = 220.0;
const NAME_GAP: f32 = 8.0;

gpui_kit::actions!(openwoot, [Undo, Redo, OpenBackgroundService]);

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
    profile_search: Entity<InputState>,
    navigation_open: bool,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct ProfileMenu {
    shell: Entity<Shell>,
    page: Option<Entity<ProfilesPage>>,
    profiles: Vec<ProfileEntry>,
    active: Option<Slot>,
    room: bool,
    search: Entity<InputState>,
}

#[derive(Clone)]
struct ProfileEntry {
    slot: Slot,
    name: SharedString,
    color: Hsla,
    icon: u8,
    shortcut: Vec<SharedString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Actions {
    Bar,
    Row,
    Floating,
}

impl Shell {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscription = cx.subscribe_in(&store, window, |this, _, event, window, cx| {
            match event {
                StoreEvent::Failed(message) => {
                    window.push_notification(Notification::error(message.clone()), cx);
                }
                StoreEvent::Disconnected => {
                    window.push_notification(Notification::warning("Keyboard Disconnected"), cx);
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
        let profile_search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let searched = cx.subscribe_in(
            &profile_search,
            window,
            |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
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
            profile_search,
            navigation_open: true,
            focus,
            _subscriptions: vec![subscription, observation, searched, refocus],
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
        panel::set_focus(false, cx);
        panel::Modal::close(cx);
        cx.notify();
    }

    fn profiles(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<ProfilesPage>> {
        let store = self.store.clone();
        self.pages
            .entry(Page::Profiles)
            .or_insert_with(|| Page::Profiles.open(&store, window, cx))
            .clone()
            .downcast::<ProfilesPage>()
            .ok()
    }

    fn open_creator(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Page::Profiles, cx);
        if let Some(profiles) = self.profiles(window, cx) {
            profiles.update(cx, |profiles, cx| profiles.open_creator(window, cx));
        }
    }

    fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Page::Profiles, cx);
        if let Some(profiles) = self.profiles(window, cx) {
            profiles.update(cx, |profiles, cx| profiles.choose_import(window, cx));
        }
    }

    fn view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let store = self.store.clone();
        self.pages
            .entry(self.page)
            .or_insert_with(|| self.page.open(&store, window, cx))
            .clone()
    }

    fn actions_placement(&self) -> Actions {
        match (self.page.group(), self.page) {
            (Group::Keyboard, _) => Actions::Bar,
            (Group::Device, Page::Switches) => Actions::Row,
            (Group::Device, _) => Actions::Floating,
        }
    }

    fn rail(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let current = self.page.group();
        let entries = [
            (Group::Keyboard, Page::Remap.icon(), "Keyboard", Page::Quick),
            (
                Group::Device,
                Page::Settings.icon(),
                "Settings",
                Page::Settings,
            ),
        ];
        v_flex()
            .flex_none()
            .w(px(RAIL_WIDTH))
            .h_full()
            .pt(px(RAIL_PADDING_Y))
            .pb(px(RAIL_PADDING_Y))
            .px(px(RAIL_PADDING_X))
            .gap(px(RAIL_LOGO_GAP))
            .items_center()
            .bg(theme.sidebar)
            .child(
                div()
                    .h(px(RAIL_LOGO_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(RAIL_LOGO))
                            .rounded(px(RADIUS))
                            .border_2()
                            .border_color(theme.foreground)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(BODY_TEXT))
                            .font_weight(FontWeight::BOLD)
                            .child("ow"),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(RAIL_ITEM_GAP))
                    .children(entries.into_iter().map(|(group, icon, label, landing)| {
                        rail_item(
                            SharedString::from(format!("rail-{label}")),
                            icon,
                            label,
                            group == current,
                            &theme,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.page.group() != group {
                                this.show(landing, cx);
                            }
                        }))
                    }))
                    .child(help_menu(&cx.entity(), &theme)),
            )
    }

    fn navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let open = self.navigation_open;
        let title = self.navigation_title(open, &theme, cx);
        let device = self.device(open, cx);
        let sections: Vec<Div> = self
            .sections(cx)
            .into_iter()
            .map(|(label, pages)| self.nav_section(label, pages, open, window, cx))
            .collect();
        div().flex_none().h_full().py(px(NAVIGATION_PADDING)).child(
            v_flex()
                .w(px(if open {
                    NAVIGATION_WIDTH
                } else {
                    COLLAPSED_WIDTH
                }))
                .h_full()
                .py(px(NAVIGATION_PADDING))
                .px(px(if open {
                    NAVIGATION_PADDING
                } else {
                    COLLAPSED_PADDING_X
                }))
                .gap(px(NAVIGATION_GAP))
                .rounded(px(RADIUS))
                .bg(theme.group_box)
                .child(title)
                .child(device)
                .child(
                    v_flex()
                        .id("navigation")
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_y_scroll()
                        .gap(px(NAVIGATION_GAP))
                        .children(sections),
                )
                .child(Self::version_footer(open, &theme)),
        )
    }

    fn sections(&self, cx: &App) -> Vec<(&'static str, Vec<Page>)> {
        let keyboard = self.store.read(cx).keyboard();
        let available = |pages: &[Page]| -> Vec<Page> {
            pages
                .iter()
                .copied()
                .filter(|page| page.is_available(keyboard))
                .collect()
        };
        match self.page.group() {
            Group::Keyboard => vec![
                ("Profiles", vec![Page::Quick, Page::Profiles]),
                ("Keyboard Configuration", available(&CONFIGURATION)),
            ],
            Group::Device => vec![
                (
                    "Keyboard Settings",
                    available(&[Page::Settings, Page::Switches]),
                ),
                ("Updates", vec![Page::Firmware]),
                ("openwoot Settings", vec![Page::Background]),
            ],
        }
    }

    fn navigation_title(&self, open: bool, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let title = match self.page.group() {
            Group::Keyboard => "Keyboard Configuration",
            Group::Device => "Settings",
        };
        let toggle = if open {
            ActionButton::icon_only("navigation-close", IconName::PanelLeftClose)
                .tooltip("Hide Navigation")
        } else {
            ActionButton::icon_only("navigation-open", IconName::PanelLeftOpen)
                .tooltip("Show Navigation")
        };
        h_flex()
            .w_full()
            .h(px(NAVIGATION_TITLE_HEIGHT))
            .gap(px(SECTION_GAP))
            .when(open, |this| {
                this.justify_between().child(
                    div()
                        .text_size(px(BODY_TEXT))
                        .line_height(px(BODY_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title),
                )
            })
            .when(!open, gpui_kit::Styled::justify_center)
            .child(toggle.on_click(cx.listener(|this, _, _, cx| {
                this.navigation_open = !this.navigation_open;
                cx.notify();
            })))
    }

    fn version_footer(open: bool, theme: &Theme) -> Div {
        div()
            .w_full()
            .text_center()
            .text_size(px(BODY_TEXT))
            .line_height(px(VERSION_LINE))
            .text_color(theme.muted_foreground)
            .child(if open {
                format!("openwoot v{VERSION}")
            } else {
                format!("v{VERSION}")
            })
    }

    fn nav_section(
        &self,
        label: &'static str,
        pages: Vec<Page>,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let theme = cx.theme().clone();
        let current = self.page;
        let items: Vec<Stateful<Div>> = pages
            .into_iter()
            .map(|page| {
                let item = nav_item(page, current == page, open, &theme)
                    .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)));
                if open {
                    return item;
                }
                panel::with_tip_right(
                    h_flex()
                        .id(SharedString::from(format!("page-slot-{}", page.title())))
                        .w_full()
                        .justify_center()
                        .child(item),
                    SharedString::from(format!("page-tip-{}", page.title())),
                    Tip::Text(page.title().into()),
                    window,
                    cx,
                )
            })
            .collect();
        v_flex()
            .w_full()
            .gap(px(SECTION_GAP))
            .when(!open, gpui_kit::Styled::items_center)
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .px(px(SECTION_LABEL_PADDING_X))
                    .py(px(SECTION_LABEL_PADDING_Y))
                    .text_size(px(SECTION_LABEL_TEXT))
                    .line_height(px(SECTION_LABEL_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if open {
                        theme.sidebar_foreground
                    } else {
                        gpui_kit::transparent_black()
                    })
                    .child(label),
            )
            .children(items)
    }

    fn device(&self, open: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let (caption, name, dot): (SharedString, SharedString, Hsla) =
            match (store.keyboard(), store.link()) {
                (Some(keyboard), _) => (
                    format!("Firmware {}", keyboard.version.firmware).into(),
                    keyboard.display_name(),
                    theme.success,
                ),
                (None, Link::Loading) => ("Connecting".into(), "Keyboard".into(), theme.warning),
                (None, Link::Unreachable(_)) => {
                    ("Unavailable".into(), "Keyboard".into(), theme.danger)
                }
                (None, Link::Searching | Link::Connected) => (
                    "Not Connected".into(),
                    "No Keyboard".into(),
                    theme.muted_foreground,
                ),
            };
        h_flex()
            .id("device")
            .w_full()
            .p(px(if open {
                DEVICE_PADDING
            } else {
                COLLAPSED_DEVICE_PADDING
            }))
            .gap(px(DEVICE_GAP))
            .rounded(px(RADIUS))
            .bg(theme.accent)
            .cursor_pointer()
            .hover(|style| style.bg(theme.secondary))
            .on_click(cx.listener(|this, _, window, cx| this.open_connection(window, cx)))
            .child(
                div()
                    .relative()
                    .flex_none()
                    .size(px(DEVICE_IMAGE))
                    .rounded(px(RADIUS))
                    .bg(theme.background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Page::Remap
                            .icon()
                            .size(px(ICON))
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(px(STATUS_INSET))
                            .right(px(STATUS_INSET))
                            .size(px(STATUS_DOT))
                            .rounded_full()
                            .bg(dot),
                    ),
            )
            .when(open, |this| {
                this.child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(DEVICE_TEXT_GAP))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(SECTION_LABEL_TEXT))
                                .line_height(px(SECTION_LABEL_LINE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.sidebar_foreground)
                                .child(caption),
                        )
                        .child(
                            h_flex()
                                .w_full()
                                .gap(px(SECTION_GAP))
                                .justify_between()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(BODY_TEXT))
                                        .line_height(px(BODY_LINE))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme.foreground)
                                        .child(name),
                                )
                                .child(
                                    Icon::new(IconName::ChevronDown)
                                        .size(px(ICON))
                                        .text_color(theme.foreground),
                                ),
                        ),
                )
            })
    }

    fn actions(&self, cx: &mut Context<Self>) -> Option<Div> {
        let store = self.store.read(cx);
        let keyboard = store.keyboard()?;
        let unsaved = keyboard.unsaved;
        let (can_undo, can_redo) = (store.can_undo(), store.can_redo());
        Some(
            h_flex()
                .gap(px(HEADER_GAP))
                .justify_end()
                .child(
                    ActionButton::icon_only("undo", IconName::Undo2)
                        .tip(Tip::Keys {
                            keys: panel::shortcut(USAGE_Z, false),
                            text: UNDO_HELP.into(),
                        })
                        .disabled(!can_undo)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, Store::undo);
                        })),
                )
                .child(
                    ActionButton::icon_only("redo", IconName::Redo2)
                        .tip(Tip::Keys {
                            keys: panel::shortcut(USAGE_Z, true),
                            text: REDO_HELP.into(),
                        })
                        .disabled(!can_redo)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, Store::redo);
                        })),
                )
                .when(unsaved, |this| {
                    this.child(
                        ActionButton::new("revert", "Discard")
                            .large()
                            .px(px(SAVE_PADDING))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.store.update(cx, |store, cx| store.revert(cx)).detach();
                            })),
                    )
                })
                .child(
                    ActionButton::new("save", "Save to Keyboard")
                        .primary()
                        .large()
                        .px(px(SAVE_PADDING))
                        .disabled(!unsaved)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.store.update(cx, |store, cx| store.save(cx)).detach();
                        })),
                ),
        )
    }

    fn header(&self, page: Option<Entity<ProfilesPage>>, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let keyboard = self.store.read(cx).keyboard();
        let profiles = profile_entries(keyboard);
        let mode = keyboard.map_or_else(Vec::new, |keyboard| {
            key_legends(&keyboard.shortcut(Action::Function(Function::ToggleModeProfile)))
        });
        let active = keyboard.map(|keyboard| keyboard.active);
        let room = keyboard.is_some_and(|keyboard| {
            keyboard.profiles.len() < usize::from(keyboard.capabilities.profiles)
        });
        let current = profiles
            .iter()
            .find(|entry| Some(entry.slot) == active)
            .cloned();
        let menu = ProfileMenu {
            shell: cx.entity(),
            page,
            profiles,
            active,
            room,
            search: self.profile_search.clone(),
        };
        let switcher = current.map(|entry| {
            Dropdown::new("profile-switcher")
                .trigger(move |open, cx| profile_trigger(&entry, &mode, open, cx))
                .content(move |close, window, cx| profile_menu(&menu, close, window, cx))
        });
        let actions = self.actions(cx);
        h_flex()
            .flex_none()
            .w_full()
            .h(px(HEADER_HEIGHT))
            .rounded(px(RADIUS))
            .bg(theme.group_box)
            .overflow_hidden()
            .child(div().flex_1().min_w_0().h_full().children(switcher))
            .child(
                div()
                    .flex_none()
                    .w(px(DIVIDER_WIDTH))
                    .h_full()
                    .bg(theme.accent.opacity(DIVIDER_OPACITY)),
            )
            .child(
                div().flex_1().min_w_0().h_full().child(
                    h_flex()
                        .size_full()
                        .px(px(HEADER_PADDING))
                        .justify_end()
                        .children(actions),
                ),
            )
    }

    fn waiting(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let store = self.store.read(cx);
        let (title, detail): (&str, SharedString) = match store.link() {
            Link::Loading => ("Connecting", SharedString::default()),
            Link::Unreachable(reason) => ("Keyboard Unavailable", reason.clone()),
            Link::Searching | Link::Connected => (
                "No Keyboard Connected",
                "Plug in a keyboard running openwoot.".into(),
            ),
        };
        let loading = matches!(store.link(), Link::Loading);
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap(px(SECTION_GAP))
            .when(loading, |this| this.child(Spinner::new()))
            .child(div().text_xl().font_weight(FontWeight::BOLD).child(title))
            .when(!detail.is_empty(), |this| {
                this.child(
                    div()
                        .max_w(px(420.0))
                        .text_center()
                        .text_color(theme.group_box_foreground)
                        .child(detail),
                )
            })
            .when(!loading, |this| {
                this.child(
                    h_flex()
                        .mt(px(HEADER_GAP))
                        .gap(px(SECTION_GAP))
                        .child(
                            ActionButton::new("waiting-help", "Troubleshoot")
                                .large()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_connection(window, cx);
                                })),
                        )
                        .child(
                            ActionButton::new("waiting-firmware", "Install Firmware")
                                .primary()
                                .large()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show(Page::Firmware, cx);
                                })),
                        ),
                )
            })
    }

    fn page_area(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let connected = self.store.read(cx).keyboard().is_some();
        if !connected && self.page.needs_keyboard() {
            return self.waiting(cx).into_any_element();
        }
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(self.view(window, cx))
            .children(panel::Modal::layer(window, cx))
            .into_any_element()
    }

    fn main_area(
        &mut self,
        placement: Actions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let content = self.page_area(window, cx);
        let top = match placement {
            Actions::Bar => {
                let page = self.profiles(window, cx);
                Some(self.header(page, cx))
            }
            Actions::Row => self.actions(cx).map(|actions| {
                h_flex()
                    .flex_none()
                    .w_full()
                    .h(px(ACTIONS_HEIGHT))
                    .justify_end()
                    .child(actions)
            }),
            Actions::Floating => None,
        };
        let floating = (placement == Actions::Floating)
            .then(|| self.actions(cx))
            .flatten()
            .map(|actions| {
                div()
                    .absolute()
                    .top(px(ACTIONS_TOP))
                    .right(px(ACTIONS_RIGHT))
                    .child(actions)
            });
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .justify_center()
            .relative()
            .child(
                panel::column()
                    .relative()
                    .children(top)
                    .child(content)
                    .children(floating),
            )
            .when(panel::focused(cx), |this| {
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(gpui_kit::black().opacity(FOCUS_DIM)),
                )
            })
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let placement = self.actions_placement();
        let header_height = match placement {
            Actions::Bar => HEADER_HEIGHT,
            Actions::Row => ACTIONS_HEIGHT,
            Actions::Floating => 0.0,
        };
        cx.set_global(Frame::new(header_height));
        let main = self.main_area(placement, window, cx);
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
            .on_action(cx.listener(|this, _: &OpenBackgroundService, _, cx| {
                this.show(Page::Background, cx);
            }))
            .flex()
            .gap(px(SHELL_GAP))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.rail(cx))
            .child(self.navigation(window, cx))
            .child(main)
    }
}

fn rail_item(
    id: SharedString,
    icon: Icon,
    label: &'static str,
    selected: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.secondary;
    v_flex()
        .id(id)
        .w_full()
        .h(px(RAIL_ITEM_HEIGHT))
        .gap(px(RAIL_ITEM_INNER_GAP))
        .items_center()
        .justify_center()
        .rounded(px(SMALL_RADIUS))
        .cursor_pointer()
        .when(selected, |this| this.bg(theme.secondary))
        .hover(move |style| style.bg(hover))
        .child(icon.size(px(ICON)).text_color(if selected {
            theme.primary
        } else {
            theme.muted_foreground
        }))
        .child(
            div()
                .whitespace_nowrap()
                .text_size(px(RAIL_LABEL_TEXT))
                .line_height(px(RAIL_LABEL_LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if selected {
                    theme.group_box_foreground
                } else {
                    theme.sidebar_foreground
                })
                .child(label),
        )
}

fn help_menu(shell: &Entity<Shell>, theme: &Theme) -> Dropdown {
    let theme = theme.clone();
    let shell = shell.clone();
    let links = [
        ("Documentation", Glyph::BookOpen, DOCUMENTATION_URL),
        ("Report a Problem", Glyph::Bug, ISSUES_URL),
        ("Source Code", Glyph::Github, SOURCE_URL),
    ];
    Dropdown::new("help")
        .menu_width(px(HELP_MENU_WIDTH))
        .trigger(move |open, _| {
            rail_item(
                "rail-help".into(),
                Icon::new(Glyph::CircleQuestionMark),
                "Help",
                open,
                &theme,
            )
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

fn nav_item(page: Page, selected: bool, open: bool, theme: &Theme) -> Stateful<Div> {
    let hover = theme.accent;
    h_flex()
        .id(SharedString::from(format!("page-{}", page.title())))
        .h(px(NAVIGATION_ITEM_HEIGHT))
        .when(open, |this| {
            this.w_full()
                .px(px(NAVIGATION_ITEM_PADDING))
                .gap(px(NAVIGATION_ITEM_GAP))
        })
        .when(!open, |this| {
            this.w(px(NAVIGATION_ITEM_HEIGHT)).justify_center()
        })
        .rounded(px(RADIUS))
        .cursor_pointer()
        .when(selected, |this| this.bg(theme.secondary))
        .when(!selected, |this| this.hover(move |style| style.bg(hover)))
        .child(page.icon().size(px(ICON)).text_color(if selected {
            theme.primary
        } else {
            theme.muted_foreground
        }))
        .when(open, |this| {
            this.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(BODY_TEXT))
                    .line_height(px(BODY_LINE))
                    .font_weight(if selected {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(theme.group_box_foreground)
                    .child(page.title()),
            )
        })
}

fn profile_entries(keyboard: Option<&Keyboard>) -> Vec<ProfileEntry> {
    keyboard.map_or_else(Vec::new, |keyboard| {
        let onboard = (0_u8..).zip(&keyboard.profiles).map(|(index, info)| {
            let slot = Slot::new(index);
            let shortcut =
                key_legends(&keyboard.shortcut(Action::Function(Function::Profile(slot))));
            (slot, info, shortcut)
        });
        let linked = (0_u8..)
            .zip(&keyboard.linked)
            .map(|(index, linked)| (Slot::linked(index), &linked.info, Vec::new()));
        onboard
            .chain(linked)
            .map(|(slot, info, shortcut)| ProfileEntry {
                slot,
                name: info.name.as_str().to_owned().into(),
                color: rgb(u32::from_be_bytes([
                    0,
                    info.color.red,
                    info.color.green,
                    info.color.blue,
                ]))
                .into(),
                icon: info.icon,
                shortcut,
            })
            .collect()
    })
}

fn profile_badge(entry: &ProfileEntry, glow: bool, cx: &App) -> Div {
    profile_avatar(entry.color, entry.icon, px(AVATAR), cx).when(glow, |this| {
        this.shadow(vec![BoxShadow {
            color: entry.color,
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(AVATAR_GLOW),
            spread_radius: px(0.0),
            inset: false,
        }])
    })
}

fn profile_trigger(
    entry: &ProfileEntry,
    mode: &[SharedString],
    open: bool,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let hover = theme.tab_active;
    let glow = linear_gradient(
        90.0,
        linear_color_stop(entry.color.opacity(GLOW_ALPHA), GLOW_START),
        linear_color_stop(entry.color.opacity(0.0), 1.0),
    );
    div()
        .relative()
        .w_full()
        .h(px(HEADER_HEIGHT))
        .rounded_l(px(RADIUS))
        .when(open, |this| this.bg(hover))
        .hover(move |style| style.bg(hover))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .h_full()
                .w(px(GLOW_WIDTH))
                .rounded_l(px(RADIUS))
                .bg(glow),
        )
        .child(
            h_flex()
                .relative()
                .size_full()
                .px(px(HEADER_PADDING))
                .gap(px(HEADER_GAP))
                .justify_between()
                .child(
                    h_flex()
                        .min_w_0()
                        .gap(px(TRIGGER_GAP))
                        .child(profile_badge(entry, true, cx))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(BODY_TEXT))
                                .line_height(px(BODY_LINE))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(entry.name.clone()),
                        ),
                )
                .child(
                    h_flex()
                        .flex_none()
                        .gap(px(panel::KBD_GAP))
                        .children(
                            mode.iter().map(|legend| {
                                panel::kbd(legend.clone(), theme.sidebar_foreground, cx)
                            }),
                        )
                        .child(
                            Icon::new(if open {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .size(px(ICON))
                            .text_color(theme.foreground),
                        ),
                ),
        )
        .into_any_element()
}

fn profile_menu(
    menu: &ProfileMenu,
    close: &Close,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let query = menu.search.read(cx).value().to_lowercase();
    let (mut cards, mut linked_cards) = (Vec::new(), Vec::new());
    for entry in menu
        .profiles
        .iter()
        .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
    {
        let card = profile_card(menu, close, entry, window, cx);
        if entry.slot.is_linked() {
            linked_cards.push(card);
        } else {
            cards.push(card);
        }
    }
    let search = profile_search(&menu.search, cx);
    let room = menu.room;
    let theme = cx.theme();
    let created = (menu.shell.clone(), close.clone());
    let imported = (menu.shell.clone(), close.clone());
    v_flex()
        .w_full()
        .child(
            h_flex()
                .w_full()
                .items_stretch()
                .px(px(MENU_PADDING))
                .pt(px(MENU_PADDING))
                .mb(px(MENU_TOP_GAP))
                .gap(px(MENU_GAP))
                .child(search)
                .child(
                    ActionButton::icon_only("profile-menu-import", Glyph::Download)
                        .filled()
                        .large()
                        .tooltip("Import Profile")
                        .on_click(move |_, window, cx| {
                            let (shell, close) = &imported;
                            close.close(cx);
                            shell.update(cx, |shell, cx| shell.open_import(window, cx));
                        }),
                )
                .child(
                    ActionButton::new("profile-menu-new", "New Profile")
                        .large()
                        .px(px(SAVE_PADDING))
                        .disabled(!room)
                        .on_click(move |_, window, cx| {
                            let (shell, close) = &created;
                            close.close(cx);
                            shell.update(cx, |shell, cx| shell.open_creator(window, cx));
                        }),
                ),
        )
        .child(
            v_flex()
                .w_full()
                .pl(px(MENU_PADDING))
                .pr(px(MENU_LIST_RIGHT))
                .pb(px(MENU_LIST_BOTTOM))
                .gap(px(CARD_GAP))
                .child(menu_section_label("ONBOARD PROFILES", theme))
                .children(cards)
                .when(!linked_cards.is_empty(), |this| {
                    this.child(menu_section_label("LINKED WITH AN APPLICATION", theme))
                        .children(linked_cards)
                }),
        )
        .into_any_element()
}

fn profile_search(search: &Entity<InputState>, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(SEARCH_HEIGHT))
        .rounded(px(SMALL_RADIUS))
        .bg(theme::palette().input)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(SEARCH_ICON_INSET))
                .flex()
                .items_center()
                .child(
                    Icon::new(IconName::Search)
                        .size(px(ICON))
                        .text_color(theme.muted_foreground),
                ),
        )
        .child(
            div()
                .size_full()
                .pl(px(SEARCH_TEXT_INSET))
                .flex()
                .items_center()
                .child(Input::new(search).large().appearance(false)),
        )
}

fn profile_card(
    menu: &ProfileMenu,
    close: &Close,
    entry: &ProfileEntry,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let slot = entry.slot;
    let current = Some(slot) == menu.active;
    let options = menu
        .page
        .as_ref()
        .map(|page| page.update(cx, |page, cx| page.profile_options(slot, cx)));
    let shortcut = shortcut_chips(entry, window, cx);
    let theme = cx.theme();
    let hover = theme.secondary;
    let picked = (menu.shell.clone(), close.clone());
    h_flex()
        .id(SharedString::from(format!("profile-card-{}", slot.get())))
        .relative()
        .flex_none()
        .w_full()
        .h(px(CARD_HEIGHT))
        .px(px(CARD_PADDING_X))
        .py(px(CARD_PADDING_Y))
        .gap(px(CARD_GAP))
        .justify_between()
        .rounded(px(RADIUS))
        .bg(if current {
            theme.secondary
        } else {
            theme.accent
        })
        .when(!current, |this| {
            this.cursor_pointer().hover(move |style| style.bg(hover))
        })
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
        .when(current, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(-OUTLINE))
                    .left(px(-OUTLINE))
                    .right(px(-OUTLINE))
                    .bottom(px(-OUTLINE))
                    .rounded(px(RADIUS + OUTLINE))
                    .border_1()
                    .border_color(theme.primary),
            )
        })
        .when(!slot.is_linked(), |this| {
            this.child(slot_badge(slot, theme))
        })
        .child(
            h_flex()
                .min_w_0()
                .gap(px(TRIGGER_GAP))
                .child(profile_badge(entry, false, cx))
                .child(
                    h_flex()
                        .min_w_0()
                        .gap(px(NAME_GAP))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(BODY_TEXT))
                                .line_height(px(BODY_LINE))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(entry.name.clone()),
                        )
                        .children(shortcut),
                ),
        )
        .child(card_trailing(slot, options, theme))
}

fn shortcut_chips(
    entry: &ProfileEntry,
    window: &mut Window,
    cx: &mut App,
) -> Option<Stateful<Div>> {
    if entry.shortcut.is_empty() {
        return None;
    }
    let theme = cx.theme();
    let chips = h_flex()
        .id(SharedString::from(format!(
            "profile-shortcut-{}",
            entry.slot.get()
        )))
        .flex_none()
        .gap(px(panel::KBD_GAP))
        .children(
            entry
                .shortcut
                .iter()
                .map(|legend| panel::kbd(legend.clone(), theme.foreground, cx)),
        );
    Some(panel::with_tip_right(
        chips,
        SharedString::from(format!("profile-shortcut-tip-{}", entry.slot.get())),
        Tip::Help {
            title: SHORTCUTS_TITLE.into(),
            text: SHORTCUTS_HELP.into(),
        },
        window,
        cx,
    ))
}

fn key_legends(actions: &[Action]) -> Vec<SharedString> {
    actions
        .iter()
        .map(|action| SharedString::from(usage::legend(*action)))
        .collect()
}

fn menu_section_label(label: &'static str, theme: &Theme) -> Div {
    div()
        .text_size(px(SECTION_LABEL_TEXT))
        .line_height(px(SECTION_LABEL_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.sidebar_foreground)
        .child(label)
}

fn slot_badge(slot: Slot, theme: &Theme) -> Div {
    div()
        .absolute()
        .top(px(SLOT_INSET))
        .left(px(SLOT_INSET))
        .size(px(SLOT_SIZE))
        .rounded(px(RADIUS))
        .bg(theme.secondary)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(SLOT_TEXT))
        .line_height(px(SLOT_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.foreground)
        .child(SharedString::from(
            (usize::from(slot.get()) + 1).to_string(),
        ))
}

fn card_trailing(slot: Slot, options: Option<Dropdown>, theme: &Theme) -> Div {
    h_flex()
        .flex_none()
        .gap(px(CARD_GAP))
        .when(ProfilesPage::is_default(slot), |this| {
            this.child(
                div()
                    .h(px(TAG_HEIGHT))
                    .px(px(TAG_PADDING))
                    .flex()
                    .items_center()
                    .rounded(px(SMALL_RADIUS))
                    .bg(theme.primary)
                    .text_size(px(TAG_TEXT))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.primary_foreground)
                    .child("DEFAULT"),
            )
        })
        .when_some(options, |this, options| {
            this.child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(options),
            )
        })
}
