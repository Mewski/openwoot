use std::collections::BTreeSet;

use gpui_kit::{
    AppContext as _, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers, ModifiersChangedEvent,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        notification::Notification,
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    keys::{Action, Layer},
};

use super::{
    catalog::{self, Category, Entry},
    keycap::{self, Look},
    quick_bind,
};
use crate::{keyboard::Dragged, panel, store::Keyboard, theme, usage};

const HEADER_HEIGHT: f32 = 40.0;
const BADGE_WIDTH: f32 = 26.0;
const BADGE_HEIGHT: f32 = 22.0;
const BADGE_RADIUS: f32 = 4.0;
const BADGE_ICON: f32 = 13.0;
const CHEVRON: f32 = 24.0;
const SEARCH_HEIGHT: f32 = 48.0;
const SEARCH_RADIUS: f32 = 8.0;
const SEARCH_ICON: f32 = 16.0;
const CODE_WIDTH: f32 = 112.0;
const SEARCH_GAP: f32 = 16.0;
const HEADER_RADIUS: f32 = 4.0;
const GRID_GAP: f32 = 8.0;
const GRID_INSET: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerEvent {
    Picked(Action),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) current: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Category(Category),
    Code,
}

pub(crate) struct ActionPicker {
    search: Entity<InputState>,
    code: Entity<InputState>,
    query: SharedString,
    entries: Vec<Entry>,
    open: BTreeSet<Section>,
    layer: Layer,
    target: Option<Target>,
    capabilities: Option<Capabilities>,
    profile_names: Vec<SharedString>,
    listening: Option<Listening>,
    focus: FocusHandle,
    code_problem: Option<SharedString>,
    flush: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy)]
struct Listening {
    modifiers: Modifiers,
    caps_lock: bool,
}

impl EventEmitter<PickerEvent> for ActionPicker {}

impl ActionPicker {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search for a character"));
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("0x65"));
        let subscriptions = vec![
            cx.subscribe(&search, |this, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = state.read(cx).value();
                    cx.notify();
                }
            }),
            cx.subscribe(&code, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && this.code_problem.take().is_some() {
                    cx.notify();
                }
            }),
        ];
        Self {
            search,
            code,
            query: SharedString::default(),
            entries: catalog::entries(0),
            open: BTreeSet::from([Section::Category(Category::Basic)]),
            layer: Layer::Base,
            target: None,
            capabilities: None,
            profile_names: Vec::new(),
            listening: None,
            focus: cx.focus_handle(),
            code_problem: None,
            flush: false,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) const fn flush(mut self) -> Self {
        self.flush = true;
        self
    }

    pub(crate) fn set_keyboard(&mut self, keyboard: &Keyboard, cx: &mut Context<Self>) {
        let slots = keyboard.capabilities.profiles;
        if self.capabilities.as_ref().map(|known| known.profiles) != Some(slots) {
            self.entries = catalog::entries(slots);
        }
        self.capabilities = Some(keyboard.capabilities.clone());
        self.profile_names = keyboard
            .profiles
            .iter()
            .map(|info| SharedString::from(info.name.as_str().to_owned()))
            .collect();
        cx.notify();
    }

    pub(crate) fn set_layer(&mut self, layer: Layer, cx: &mut Context<Self>) {
        if self.layer != layer {
            self.layer = layer;
            cx.notify();
        }
    }

    pub(crate) fn set_target(&mut self, target: Option<Target>, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        self.target = target;
        if target.is_none() {
            self.listening = None;
        }
        cx.notify();
    }

    pub(crate) fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.listening = Some(Listening {
            modifiers: window.modifiers(),
            caps_lock: window.capslock().on,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn stop_listening(&mut self, cx: &mut Context<Self>) {
        if self.listening.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn refusal(&self, action: Action) -> Option<String> {
        catalog::refusal(action, self.layer).or_else(|| {
            self.capabilities
                .as_ref()
                .and_then(|capabilities| action.validate(capabilities).err())
                .map(|invalid| capitalized(&invalid.to_string()))
        })
    }

    fn toggle(&mut self, section: Section, cx: &mut Context<Self>) {
        if !self.open.remove(&section) {
            self.open.insert(section);
        }
        cx.notify();
    }

    fn pick(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.target.is_none() {
            return;
        }
        if let Some(reason) = self.refusal(action) {
            window.push_notification(Notification::warning(reason), cx);
            return;
        }
        self.listening = None;
        cx.emit(PickerEvent::Picked(action));
        cx.notify();
    }

    fn captured_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.listening.is_none() || !self.focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            self.listening = None;
            cx.emit(PickerEvent::Cancelled);
            cx.notify();
            return;
        }
        if let Some(code) = quick_bind::usage_of_key(key) {
            self.pick(Action::Keyboard(code), window, cx);
        } else {
            window.push_notification(
                Notification::warning(format!("Pick {key} from the list")),
                cx,
            );
        }
    }

    fn captured_modifiers(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.focus.is_focused(window) {
            return;
        }
        let Some(listening) = &mut self.listening else {
            return;
        };
        let pressed = if event.capslock.on == listening.caps_lock {
            quick_bind::pressed_modifier(listening.modifiers, event.modifiers)
        } else {
            Some(quick_bind::CAPS_LOCK)
        };
        listening.modifiers = event.modifiers;
        listening.caps_lock = event.capslock.on;
        if let Some(code) = pressed {
            cx.stop_propagation();
            self.pick(Action::Keyboard(code), window, cx);
        }
    }

    fn bind_code(&mut self, consumer: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.code.read(cx).value();
        let digits = text.trim();
        let digits = digits
            .strip_prefix("0x")
            .or_else(|| digits.strip_prefix("0X"))
            .unwrap_or(digits);
        let action = u16::from_str_radix(digits, 16).ok().and_then(|code| {
            if consumer {
                Some(Action::Consumer(code))
            } else {
                u8::try_from(code).ok().map(Action::Keyboard)
            }
        });
        let Some(action) = action else {
            self.code_problem = Some("Enter a hexadecimal usage, such as 0x65".into());
            cx.notify();
            return;
        };
        if let Some(reason) = self.refusal(action) {
            self.code_problem = Some(reason.into());
            cx.notify();
            return;
        }
        self.code_problem = None;
        self.pick(action, window, cx);
    }

    fn sections(&self, cx: &mut Context<Self>) -> Div {
        let mut list = v_flex();
        for category in Category::ALL {
            let section = Section::Category(category);
            let open = self.open.contains(&section);
            list = list.child(self.header(section, open, cx));
            if open {
                let entries: Vec<Entry> = self
                    .entries
                    .iter()
                    .filter(|entry| entry.category == category)
                    .copied()
                    .collect();
                list = list.child(self.grid(&entries, cx));
            }
        }
        let open = self.open.contains(&Section::Code);
        list.child(self.header(Section::Code, open, cx))
            .when(open, |this| this.child(self.code_row(cx)))
    }

    fn results(&self, cx: &mut Context<Self>) -> Div {
        let found = catalog::search(&self.entries, &self.query);
        if found.is_empty() {
            return no_results(cx);
        }
        let mut list = v_flex();
        for category in Category::ALL {
            let entries: Vec<Entry> = found
                .iter()
                .filter(|entry| entry.category == category)
                .copied()
                .collect();
            if entries.is_empty() {
                continue;
            }
            list = list
                .child(self.header(Section::Category(category), true, cx))
                .child(self.grid(&entries, cx));
        }
        list
    }

    fn header(&self, section: Section, open: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let (badge, chevron, hover) = (
            theme.secondary,
            theme.group_box_foreground,
            theme.tab_active,
        );
        let (title, mark) = match section {
            Section::Category(category) => (category.title(), mark(category)),
            Section::Code => ("HID usage", Mark::Icon(Catalog::SquareTerminal)),
        };
        let searching = self.is_searching();
        h_flex()
            .id(SharedString::from(format!("section-{title}")))
            .w_full()
            .h(px(HEADER_HEIGHT))
            .px_4()
            .gap_3()
            .items_center()
            .rounded(px(HEADER_RADIUS))
            .child(
                h_flex()
                    .flex_none()
                    .w(px(BADGE_WIDTH))
                    .h(px(BADGE_HEIGHT))
                    .items_center()
                    .justify_center()
                    .rounded(px(BADGE_RADIUS))
                    .bg(badge)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .child(match mark {
                        Mark::Letter(letter) => div().child(letter).into_any_element(),
                        Mark::Icon(icon) => Icon::new(icon).size(px(BADGE_ICON)).into_any_element(),
                    }),
            )
            .child(div().flex_1().text_base().child(title))
            .when(!searching, |this| {
                this.cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .child(
                        div().text_color(chevron).child(
                            Icon::new(if open {
                                Catalog::ChevronUp
                            } else {
                                Catalog::ChevronDown
                            })
                            .size(px(CHEVRON)),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(section, cx)))
            })
    }

    fn grid(&self, entries: &[Entry], cx: &mut Context<Self>) -> Div {
        let tiles: Vec<Stateful<Div>> = entries
            .iter()
            .map(|entry| self.tile(entry.action, cx))
            .collect();
        h_flex()
            .flex_wrap()
            .gap(px(GRID_GAP))
            .pl(px(GRID_INSET))
            .pr(px(GRID_INSET + GRID_GAP))
            .pt(px(GRID_GAP))
            .pb(px(GRID_INSET + GRID_GAP))
            .children(tiles)
    }

    fn tile(&self, action: Action, cx: &mut Context<Self>) -> Stateful<Div> {
        let refusal = self.refusal(action);
        let current = self
            .target
            .is_some_and(|target| target.current == action && action != Action::None);
        let look = Look {
            current,
            disabled: refusal.is_some(),
        };
        let profile_name = match action {
            Action::Function(openwoot_protocol::keys::Function::Profile(slot)) => self
                .profile_names
                .get(usize::from(slot.get()))
                .map(SharedString::as_str),
            _ => None,
        };
        let tooltip: Option<SharedString> = refusal
            .clone()
            .or_else(|| catalog::help(action, profile_name))
            .map(SharedString::from);
        let caps = theme::keycaps();
        let accent = cx.theme().primary;
        let tile = keycap::tile(
            ElementId::Name(format!("pick-{action:?}").into()),
            action,
            look,
            cx,
        )
        .when_some(tooltip, |this, text| {
            this.tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
        });
        if refusal.is_some() {
            return tile;
        }
        let mut dragged = Dragged::new(action, SharedString::from(usage::legend(action)));
        if let Some(glyph) = keycap::glyph(action, caps.legend, accent) {
            dragged = dragged.glyph(glyph);
        }
        tile.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .on_click(cx.listener(move |this, _, window, cx| this.pick(action, window, cx)))
    }

    fn code_row(&self, cx: &mut Context<Self>) -> Div {
        let danger = cx.theme().danger;
        let idle = self.target.is_none();
        h_flex()
            .gap_2()
            .items_center()
            .pt_1()
            .pb_4()
            .child(
                div()
                    .w(px(CODE_WIDTH))
                    .child(Input::new(&self.code).small()),
            )
            .child(
                panel::ActionButton::new("bind-keyboard-code", "Keyboard")
                    .disabled(idle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.bind_code(false, window, cx);
                    })),
            )
            .child(
                panel::ActionButton::new("bind-consumer-code", "Consumer")
                    .disabled(idle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.bind_code(true, window, cx);
                    })),
            )
            .when_some(self.code_problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(danger).child(problem))
            })
    }

    fn is_searching(&self) -> bool {
        !self.query.trim().is_empty()
    }
}

impl Render for ActionPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("action-picker")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::captured_key))
            .on_modifiers_changed(cx.listener(Self::captured_modifiers))
            .w_full()
            .gap(px(if self.flush { 0.0 } else { SEARCH_GAP }))
            .child(
                Input::new(&self.search)
                    .large()
                    .cleanable(true)
                    .prefix(
                        Icon::new(Catalog::Search)
                            .size(px(SEARCH_ICON))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .h(px(SEARCH_HEIGHT))
                    .text_sm()
                    .rounded(px(SEARCH_RADIUS))
                    .bg(cx.theme().sidebar)
                    .border_color(cx.theme().border),
            )
            .child(if self.is_searching() {
                self.results(cx)
            } else {
                self.sections(cx)
            })
    }
}

#[derive(Debug, Clone, Copy)]
enum Mark {
    Letter(&'static str),
    Icon(Catalog),
}

const fn mark(category: Category) -> Mark {
    match category {
        Category::Basic => Mark::Letter("A"),
        Category::Extended => Mark::Letter("#"),
        Category::Functions => Mark::Icon(Catalog::Settings),
        Category::Profiles => Mark::Icon(Catalog::UserRound),
        Category::Media => Mark::Icon(Catalog::Play),
        Category::Mouse => Mark::Icon(Catalog::Mouse),
        Category::Gamepad => Mark::Icon(Catalog::Gamepad2),
        Category::OtherKeys => Mark::Icon(Catalog::Keyboard),
    }
}

fn no_results(cx: &Context<ActionPicker>) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap_4()
        .pt_2()
        .child(
            div()
                .w_full()
                .text_center()
                .text_color(theme.muted_foreground)
                .child("No results found"),
        )
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap_3()
                .p_4()
                .rounded(theme.radius)
                .bg(theme.secondary)
                .child(
                    Icon::new(Catalog::TriangleAlert)
                        .text_color(theme.warning)
                        .flex_none(),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Special characters can't be remapped"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("Your computer's language decides these characters"),
                        ),
                ),
        )
}

fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}
