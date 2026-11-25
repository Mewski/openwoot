use std::collections::{HashMap, HashSet};

use gpui_kit::{
    AnyElement, App, AppContext as _, Context, Div, Entity, EventEmitter, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, WeakEntity, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, h_flex,
        input::{InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    apps::{App as LinkedApp, Custom, Filter, Link, MAX_FILTERS, MAX_LINKS, Matching, Text},
    profile::Slot,
};

use super::card::{count_chip, tipped};
use crate::{
    app_watcher::{self, InstalledGame, RunningApp},
    background::Background,
    dropdown::{Close, Dropdown},
    panel::{self, ActionButton, Tip},
    store::Store,
};

const LIST_GAP: f32 = 12.0;
const BLOCK_GAP: f32 = 16.0;
const HEADER_GAP: f32 = 8.0;
const ROW_HEIGHT: f32 = 40.0;
const ROW_PADDING_X: f32 = 12.0;
const ROW_GAP: f32 = 8.0;
const ROW_RADIUS: f32 = 6.0;
const ROW_ICON: f32 = 20.0;
const ROW_TEXT: f32 = 16.0;
const ROW_LINE: f32 = 19.2;
const PICKER_PADDING: f32 = 8.0;
const PICKER_GAP: f32 = 8.0;
const CHOICES_HEIGHT: f32 = 264.0;
const CHOICE_HEIGHT: f32 = 36.0;
const GROUP_TEXT: f32 = 12.0;
const GROUP_LINE: f32 = 14.4;
const GROUP_PADDING: f32 = 8.0;
const FORM_PADDING: f32 = 16.0;
const FORM_GAP: f32 = 20.0;
const METHODS_GAP: f32 = 24.0;
const METHOD_GAP: f32 = 12.0;
const BOX_SIZE: f32 = 18.0;
const BODY_LINE: f32 = 22.4;
const TITLE_GAP: f32 = 2.0;
const GROUP_BOTTOM: f32 = 4.0;
const BOX_NUDGE: f32 = 1.0;
const DISABLED_OPACITY: f32 = 0.5;
const BOX_RADIUS: f32 = 4.0;
const BOX_CHECK: f32 = 14.0;
const SMALL_TEXT: f32 = 14.0;
const SMALL_LINE: f32 = 19.6;
const STATUS_ICON: f32 = 16.0;
const DESCRIPTION: &str = "Your device will automatically switch to this profile when a linked application is your active window.";
const RUNNING: &str = "Background service is running.";
const NOT_RUNNING: &str = "Background service is not running.";

pub(super) struct AppList {
    store: Entity<Store>,
    profile: Option<Slot>,
    links: Vec<Link>,
    picker: Entity<AppPicker>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppListEvent {
    Changed,
}

pub(super) struct AppPicker {
    list: WeakEntity<AppList>,
    search: Entity<InputState>,
    name: Entity<InputState>,
    methods: Vec<Method>,
    kept: Vec<Filter>,
    mode: Mode,
    target: Option<usize>,
    close: Option<Close>,
    running: Vec<RunningApp>,
    installed: Vec<InstalledGame>,
    _subscriptions: Vec<Subscription>,
}

struct Method {
    kind: MethodKind,
    input: Entity<InputState>,
    checked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MethodKind {
    App,
    Folder,
    Title,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Search,
    Custom,
}

#[derive(Clone)]
struct Choice {
    name: SharedString,
    link: Link,
    steam: bool,
}

impl EventEmitter<AppListEvent> for AppList {}

impl AppList {
    pub(super) fn new(
        store: Entity<Store>,
        profile: Option<Slot>,
        links: Vec<Link>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let list = cx.entity().downgrade();
        let picker = cx.new(|cx| AppPicker::new(list, window, cx));
        let mut subscriptions = vec![cx.observe(&store, |_, _, cx| cx.notify())];
        subscriptions.extend(
            Background::global(cx)
                .map(|background| cx.observe(&background, |_, _, cx| cx.notify())),
        );
        Self {
            store,
            profile,
            links,
            picker,
            _subscriptions: subscriptions,
        }
    }

    pub(super) fn links(&self) -> &[Link] {
        &self.links
    }

    fn put(&mut self, target: Option<usize>, link: Link, cx: &mut Context<Self>) {
        let count = self.links.len();
        match target.filter(|index| *index < count) {
            Some(index) => {
                if let Some(slot) = self.links.get_mut(index) {
                    *slot = link;
                }
            }
            None if count < usize::from(MAX_LINKS) => self.links.push(link),
            None => return,
        }
        cx.emit(AppListEvent::Changed);
        cx.notify();
    }

    fn remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.links.len() {
            self.links.remove(index);
            cx.emit(AppListEvent::Changed);
            cx.notify();
        }
    }

    fn used_by(&self, cx: &App) -> HashMap<String, SharedString> {
        let mut used = HashMap::new();
        if let Some(keyboard) = self.store.read(cx).keyboard() {
            for (index, linked) in (0_u8..).zip(&keyboard.linked) {
                if Some(Slot::linked(index)) == self.profile {
                    continue;
                }
                for link in &linked.links {
                    used.entry(app_watcher::identity(link))
                        .or_insert_with(|| linked.info.name.as_str().to_owned().into());
                }
            }
        }
        used
    }

    fn row(&self, index: usize, link: &Link) -> AnyElement {
        let picker = self.picker.clone();
        let name = display_name(link);
        let steam = matches!(link.app, LinkedApp::Steam(_));
        Dropdown::new(("linked-app", index))
            .content(move |close, window, cx| {
                picker.update(cx, |picker, cx| {
                    picker.open(Some(index), close.clone(), window, cx);
                });
                picker.clone().into_any_element()
            })
            .trigger(move |open, cx| app_face(&name, steam, open, cx))
            .into_any_element()
    }

    fn add_button(&self) -> AnyElement {
        let picker = self.picker.clone();
        let label = if self.links.is_empty() {
            "Link an application"
        } else {
            "Link another application"
        };
        Dropdown::new("link-app")
            .content(move |close, window, cx| {
                picker.update(cx, |picker, cx| {
                    picker.open(None, close.clone(), window, cx);
                });
                picker.clone().into_any_element()
            })
            .trigger(move |open, _| {
                ActionButton::new("link-app-face", label)
                    .icon(Icon::new(Catalog::Plus))
                    .selected(open)
                    .into_any_element()
            })
            .into_any_element()
    }
}

impl Render for AppList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let running =
            Background::global(cx).is_some_and(|background| background.read(cx).is_running());
        let rows: Vec<AnyElement> = self
            .links
            .iter()
            .enumerate()
            .map(|(index, link)| self.row(index, link))
            .collect();
        let room = self.links.len() < usize::from(MAX_LINKS);
        v_flex()
            .w_full()
            .gap(px(BLOCK_GAP))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(HEADER_GAP))
                    .child(
                        h_flex()
                            .gap(px(HEADER_GAP))
                            .child(panel::label("Linked Applications", cx))
                            .child(count_chip(self.links.len(), usize::from(MAX_LINKS), cx)),
                    )
                    .child(
                        div()
                            .text_size(px(ROW_TEXT))
                            .line_height(px(BODY_LINE))
                            .text_color(theme.group_box_foreground)
                            .child(DESCRIPTION),
                    ),
            )
            .child(
                v_flex()
                    .w_full()
                    .gap(px(LIST_GAP))
                    .children(rows)
                    .when(room, |this| this.child(self.add_button())),
            )
            .child(status_line(running, cx))
    }
}

impl AppPicker {
    fn new(list: WeakEntity<AppList>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let name = cx.new(|cx| InputState::new(window, cx));
        let methods: Vec<Method> = [MethodKind::App, MethodKind::Folder, MethodKind::Title]
            .into_iter()
            .map(|kind| Method {
                kind,
                input: cx.new(|cx| InputState::new(window, cx)),
                checked: false,
            })
            .collect();
        let mut subscriptions = vec![cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })];
        subscriptions.extend(methods.iter().map(|method| {
            cx.subscribe(&method.input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
        }));
        let installed = cx.background_spawn(async { app_watcher::installed_games() });
        cx.spawn(async move |this, cx| {
            let installed = installed.await;
            this.update(cx, |picker, cx| {
                picker.installed = installed;
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self {
            list,
            search,
            name,
            methods,
            kept: Vec::new(),
            mode: Mode::Search,
            target: None,
            close: None,
            running: Vec::new(),
            installed: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn open(
        &mut self,
        target: Option<usize>,
        close: Close,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let fresh = self.close.is_none() || self.target != target;
        self.close = Some(close);
        if fresh {
            self.target = target;
            self.mode = Mode::Search;
            self.running = app_watcher::running_apps();
            self.search
                .update(cx, |search, cx| search.set_value("", window, cx));
        }
    }

    fn current(&self, cx: &App) -> Option<Link> {
        let list = self.list.upgrade()?;
        let index = self.target?;
        list.read(cx).links.get(index).cloned()
    }

    fn choose(&mut self, link: Link, cx: &mut Context<Self>) {
        let target = self.target;
        self.list
            .update(cx, |list, cx| list.put(target, link, cx))
            .ok();
        self.finish(cx);
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.target {
            self.list.update(cx, |list, cx| list.remove(index, cx)).ok();
        }
        self.finish(cx);
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        if let Some(close) = self.close.take() {
            close.close(cx);
        }
        self.mode = Mode::Search;
        cx.notify();
    }

    fn customise(&mut self, link: Option<&Link>, window: &mut Window, cx: &mut Context<Self>) {
        let custom = link.and_then(|link| match &link.app {
            LinkedApp::Custom(custom) => Some((link.name.as_str().to_owned(), custom.clone())),
            LinkedApp::Steam(_) => None,
        });
        let (name, filters) = custom.map_or_else(
            || (String::new(), Vec::new()),
            |(name, custom)| (name, custom.filters.into_iter().collect()),
        );
        self.name
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.kept = filters
            .iter()
            .filter(|filter| matches!(filter, Filter::ProcessPath(_)))
            .cloned()
            .collect();
        for method in &mut self.methods {
            let value = filters.iter().find_map(|filter| method.kind.value(filter));
            method.checked = value.is_some();
            let text = value.unwrap_or_default();
            method
                .input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
        self.mode = Mode::Custom;
        cx.notify();
    }

    fn custom_link(&self, cx: &App) -> Option<Link> {
        let mut filters: Vec<Filter> = self.kept.clone();
        for method in self.methods.iter().filter(|method| method.checked) {
            let value = method.input.read(cx).value();
            let text = Text::try_from(value.trim())
                .ok()
                .filter(|text| !text.is_empty())?;
            filters.push(method.kind.filter(text));
        }
        if filters.is_empty() || filters.len() > MAX_FILTERS {
            return None;
        }
        let name = self.name.read(cx).value();
        Some(Link {
            name: Text::try_from(name.trim()).ok()?,
            app: LinkedApp::Custom(Custom {
                matching: Matching::All,
                filters: filters.into_iter().collect(),
            }),
        })
    }

    fn choices(&self, cx: &App) -> (Vec<Choice>, Vec<Choice>) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let wanted = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let running = self
            .running
            .iter()
            .filter(|app| wanted(&app.name))
            .filter_map(|app| {
                Some(Choice {
                    name: app.name.clone().into(),
                    link: app.link()?,
                    steam: false,
                })
            })
            .collect();
        let installed = self
            .installed
            .iter()
            .filter(|game| wanted(&game.name))
            .filter_map(|game| {
                Some(Choice {
                    name: game.name.clone().into(),
                    link: game.link()?,
                    steam: true,
                })
            })
            .collect();
        (running, installed)
    }

    fn search_view(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let (taken, used_by) = self.list.upgrade().map_or_else(
            || (HashSet::new(), HashMap::new()),
            |list| {
                let list = list.read(cx);
                let taken = list
                    .links
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| Some(*index) != self.target)
                    .map(|(_, link)| app_watcher::identity(link))
                    .collect();
                (taken, list.used_by(cx))
            },
        );
        let (running, installed) = self.choices(cx);
        let empty = running.is_empty() && installed.is_empty();
        let mut groups = v_flex().w_full();
        for (label, choices) in [("Running", running), ("Installed", installed)] {
            if choices.is_empty() {
                continue;
            }
            groups = groups.child(group_label(label, cx));
            for (index, choice) in choices.into_iter().enumerate() {
                let identity = app_watcher::identity(&choice.link);
                let disabled = taken.contains(&identity);
                let owner = used_by.get(&identity).cloned();
                groups = groups.child(Self::choice_row(
                    (label, index),
                    choice,
                    disabled,
                    owner,
                    cx,
                ));
            }
        }
        let current = self.current(cx);
        let editable = current
            .as_ref()
            .is_some_and(|link| matches!(link.app, LinkedApp::Custom(_)));
        v_flex()
            .w_full()
            .p(px(PICKER_PADDING))
            .gap(px(PICKER_GAP))
            .child(panel::text_field(&self.search, "Search...", window, cx))
            .child(
                div()
                    .id("app-choices")
                    .w_full()
                    .max_h(px(CHOICES_HEIGHT))
                    .overflow_y_scroll()
                    .child(if empty {
                        div()
                            .px(px(ROW_PADDING_X))
                            .py(px(GROUP_PADDING))
                            .text_size(px(SMALL_TEXT))
                            .text_color(theme.muted_foreground)
                            .child("No matching apps")
                            .into_any_element()
                    } else {
                        groups.into_any_element()
                    }),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap(px(PICKER_GAP))
                    .justify_between()
                    .child(
                        ActionButton::new("app-custom", "App not listed? Add it manually.")
                            .icon(Icon::new(Catalog::Pencil))
                            .plain()
                            .on_click(cx.listener(|picker, _, window, cx| {
                                picker.customise(None, window, cx);
                            })),
                    )
                    .child(
                        h_flex()
                            .gap(px(PICKER_GAP))
                            .when(editable, |this| {
                                this.child(ActionButton::new("app-edit", "Edit").on_click(
                                    cx.listener(move |picker, _, window, cx| {
                                        let link = picker.current(cx);
                                        picker.customise(link.as_ref(), window, cx);
                                    }),
                                ))
                            })
                            .when(current.is_some(), |this| {
                                this.child(
                                    ActionButton::new("app-remove", "Remove").danger().on_click(
                                        cx.listener(|picker, _, _, cx| picker.remove(cx)),
                                    ),
                                )
                            }),
                    ),
            )
    }

    fn choice_row(
        key: (&'static str, usize),
        choice: Choice,
        disabled: bool,
        owner: Option<SharedString>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let hover = theme.secondary;
        let link = choice.link.clone();
        let row = h_flex()
            .id(SharedString::from(format!(
                "app-choice-{}-{}",
                key.0, key.1
            )))
            .w_full()
            .h(px(CHOICE_HEIGHT))
            .px(px(ROW_PADDING_X))
            .gap(px(ROW_GAP))
            .rounded(px(ROW_RADIUS))
            .text_size(px(ROW_TEXT))
            .line_height(px(ROW_LINE))
            .text_color(theme.foreground)
            .when(disabled, |this| this.opacity(DISABLED_OPACITY))
            .when(!disabled, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(cx.listener(move |picker, _, _, cx| picker.choose(link.clone(), cx)))
            })
            .child(app_icon(choice.steam, cx))
            .child(div().flex_1().min_w_0().truncate().child(choice.name));
        match owner {
            Some(profile) => row.child(tipped(
                div()
                    .id(SharedString::from(format!("app-owner-{}-{}", key.0, key.1)))
                    .flex_none()
                    .text_color(theme.muted_foreground)
                    .child(Icon::new(Catalog::Info).size(px(STATUS_ICON))),
                SharedString::from(format!("app-owner-tip-{}-{}", key.0, key.1)),
                Tip::Text(
                    format!("Already linked to {profile}, selecting it moves the link here.")
                        .into(),
                ),
            )),
            None => row,
        }
    }

    fn method_row(&self, index: usize, window: &Window, cx: &Context<Self>) -> Option<Div> {
        let theme = cx.theme();
        let method = self.methods.get(index)?;
        let checked = method.checked;
        let kind = method.kind;
        Some(
            v_flex()
                .w_full()
                .gap(px(METHOD_GAP))
                .child(
                    h_flex()
                        .id(("detection-method", index))
                        .w_full()
                        .items_start()
                        .gap(px(METHOD_GAP))
                        .cursor_pointer()
                        .on_click(cx.listener(move |picker, _, _, cx| {
                            if let Some(method) = picker.methods.get_mut(index) {
                                method.checked = !method.checked;
                                cx.notify();
                            }
                        }))
                        .child(check_box(checked, cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(px(TITLE_GAP))
                                .child(
                                    div()
                                        .text_size(px(ROW_TEXT))
                                        .line_height(px(ROW_LINE))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(kind.title()),
                                )
                                .child(
                                    div()
                                        .text_size(px(SMALL_TEXT))
                                        .line_height(px(SMALL_LINE))
                                        .text_color(theme.group_box_foreground)
                                        .child(format!(
                                            "{} {}",
                                            kind.description(),
                                            kind.example()
                                        )),
                                ),
                        ),
                )
                .when(checked, |this| {
                    this.child(panel::text_field(
                        &method.input,
                        kind.placeholder(),
                        window,
                        cx,
                    ))
                }),
        )
    }

    fn custom_view(&self, window: &Window, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let valid = self.custom_link(cx).is_some();
        let methods =
            (0..self.methods.len()).filter_map(|index| self.method_row(index, window, cx));
        v_flex()
            .w_full()
            .p(px(FORM_PADDING))
            .gap(px(FORM_GAP))
            .child(panel::field(
                "App name",
                panel::text_field(&self.name, "e.g. My App", window, cx),
                cx,
            ))
            .child(
                v_flex()
                    .w_full()
                    .gap(px(HEADER_GAP))
                    .child(panel::label("Detection method", cx))
                    .child(
                        div()
                            .text_size(px(SMALL_TEXT))
                            .line_height(px(SMALL_LINE))
                            .text_color(theme.group_box_foreground)
                            .child("Choose one or more ways to detect this app."),
                    ),
            )
            .child(v_flex().w_full().gap(px(METHODS_GAP)).children(methods))
            .child(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap(px(PICKER_GAP))
                    .child(
                        ActionButton::new("app-back", "Back to search")
                            .icon(Icon::new(Catalog::ArrowLeft))
                            .on_click(cx.listener(|picker, _, _, cx| {
                                picker.mode = Mode::Search;
                                cx.notify();
                            })),
                    )
                    .child(
                        ActionButton::new("app-confirm", "Confirm")
                            .primary()
                            .disabled(!valid)
                            .on_click(cx.listener(|picker, _, _, cx| {
                                if let Some(link) = picker.custom_link(cx) {
                                    picker.choose(link, cx);
                                }
                            })),
                    ),
            )
    }
}

impl Render for AppPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.mode {
            Mode::Search => self.search_view(window, cx),
            Mode::Custom => self.custom_view(window, cx),
        }
    }
}

impl MethodKind {
    const fn title(self) -> &'static str {
        match self {
            Self::App => "A specific app",
            Self::Folder => "All apps from a folder",
            Self::Title => "By window title",
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::App => "Detect by its executable file (.exe).",
            Self::Folder => "Detect any app running from a specific folder.",
            Self::Title => "Detect when the window title contains specific text.",
        }
    }

    const fn example(self) -> &'static str {
        match self {
            Self::App => "e.g. Valorant.exe",
            Self::Folder => "e.g. all games in your Steam library",
            Self::Title => "e.g. Minecraft mods, custom launchers",
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::App => "e.g. Valorant.exe",
            Self::Folder => "e.g. C:\\Program Files\\AppName",
            Self::Title => "e.g. Minecraft",
        }
    }

    fn value(self, filter: &Filter) -> Option<String> {
        match (self, filter) {
            (Self::App, Filter::ProcessName(text))
            | (Self::Folder, Filter::ProcessDirectory(text))
            | (Self::Title, Filter::WindowTitle(text)) => Some(text.as_str().to_owned()),
            _ => None,
        }
    }

    const fn filter(self, text: Text) -> Filter {
        match self {
            Self::App => Filter::ProcessName(text),
            Self::Folder => Filter::ProcessDirectory(text),
            Self::Title => Filter::WindowTitle(text),
        }
    }
}

pub(super) fn display_name(link: &Link) -> SharedString {
    if !link.name.trim().is_empty() {
        return link.name.as_str().to_owned().into();
    }
    match &link.app {
        LinkedApp::Steam(id) => format!("Steam App {id}").into(),
        LinkedApp::Custom(custom) => custom.filters.first().map_or_else(
            || SharedString::from("Unknown App"),
            |filter| {
                let (Filter::ProcessName(text)
                | Filter::ProcessDirectory(text)
                | Filter::WindowTitle(text)
                | Filter::ProcessPath(text)) = filter;
                text.as_str().to_owned().into()
            },
        ),
    }
}

pub(super) fn summary(links: &[Link]) -> Option<SharedString> {
    let first = display_name(links.first()?);
    Some(match links.len() {
        1 => first,
        2 => format!("{first} + 1 app").into(),
        count => format!("{first} + {} apps", count - 1).into(),
    })
}

fn app_face(name: &SharedString, steam: bool, open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .h(px(ROW_HEIGHT))
        .px(px(ROW_PADDING_X))
        .gap(px(ROW_GAP))
        .rounded(px(ROW_RADIUS))
        .border_1()
        .border_color(if open { theme.primary } else { theme.border })
        .bg(if open { theme.secondary } else { theme.accent })
        .text_size(px(ROW_TEXT))
        .line_height(px(ROW_LINE))
        .font_weight(FontWeight::BOLD)
        .text_color(theme.foreground)
        .child(app_icon(steam, cx))
        .child(div().flex_1().min_w_0().truncate().child(name.clone()))
        .child(
            Icon::new(Catalog::ChevronDown)
                .size(px(ROW_ICON))
                .text_color(theme.muted_foreground),
        )
        .into_any_element()
}

fn app_icon(steam: bool, cx: &App) -> Icon {
    Icon::new(if steam {
        Catalog::Gamepad2
    } else {
        Catalog::Monitor
    })
    .size(px(ROW_ICON))
    .text_color(cx.theme().muted_foreground)
}

fn group_label(label: &'static str, cx: &App) -> Div {
    div()
        .px(px(ROW_PADDING_X))
        .pt(px(GROUP_PADDING))
        .pb(px(GROUP_BOTTOM))
        .text_size(px(GROUP_TEXT))
        .line_height(px(GROUP_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().sidebar_foreground)
        .child(SharedString::from(label.to_uppercase()))
}

fn check_box(checked: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .mt(px(BOX_NUDGE))
        .size(px(BOX_SIZE))
        .rounded(px(BOX_RADIUS))
        .border_1()
        .border_color(if checked { theme.primary } else { theme.border })
        .bg(if checked { theme.primary } else { theme.accent })
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |this| {
            this.child(
                Icon::new(Catalog::Check)
                    .size(px(BOX_CHECK))
                    .text_color(theme.primary_foreground),
            )
        })
}

fn status_line(running: bool, cx: &App) -> Div {
    let theme = cx.theme();
    let (icon, color, text) = if running {
        (Catalog::CircleCheck, theme.success, RUNNING)
    } else {
        (Catalog::TriangleAlert, theme.warning, NOT_RUNNING)
    };
    h_flex()
        .w_full()
        .gap(px(ROW_GAP))
        .text_size(px(SMALL_TEXT))
        .line_height(px(SMALL_LINE))
        .text_color(theme.group_box_foreground)
        .child(Icon::new(icon).size(px(STATUS_ICON)).text_color(color))
        .child(div().flex_1().min_w_0().child(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Text {
        Text::try_from(value).unwrap_or_default()
    }

    fn steam(name: &str, id: u32) -> Link {
        Link {
            name: text(name),
            app: LinkedApp::Steam(id),
        }
    }

    #[test]
    fn a_card_names_the_first_app_and_counts_the_rest() {
        assert_eq!(summary(&[]), None);
        assert_eq!(
            summary(&[steam("Counter-Strike 2", 730)]).as_deref(),
            Some("Counter-Strike 2")
        );
        assert_eq!(
            summary(&[steam("A", 1), steam("B", 2)]).as_deref(),
            Some("A + 1 app")
        );
        assert_eq!(
            summary(&[steam("A", 1), steam("B", 2), steam("C", 3)]).as_deref(),
            Some("A + 2 apps")
        );
    }

    #[test]
    fn an_unnamed_app_shows_its_first_detection_method() {
        let custom = Link {
            name: text(""),
            app: LinkedApp::Custom(Custom {
                matching: Matching::All,
                filters: [Filter::ProcessName(text("VALORANT.exe"))]
                    .into_iter()
                    .collect(),
            }),
        };
        assert_eq!(display_name(&custom).as_ref(), "VALORANT.exe");
        assert_eq!(display_name(&steam("", 730)).as_ref(), "Steam App 730");
    }
}
