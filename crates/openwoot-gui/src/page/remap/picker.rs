use gpui_kit::{
    AppContext as _, Context, Div, Entity, EventEmitter, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers, ModifiersChangedEvent,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    component::{
        ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
        WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        notification::Notification,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    configuration::{Capabilities, Validate as _},
    keys::{Action, Function, Layer},
};

use super::{
    catalog::{self, Category, Entry},
    quick_bind,
};
use crate::{store::Keyboard, usage};

const TILE_SIZE: f32 = 44.0;
const WIDE_TILE_HEIGHT: f32 = 36.0;
const GROUP_HEIGHT: f32 = 40.0;
const GROUP_ICON_SIZE: f32 = 26.0;
const CODE_WIDTH: f32 = 112.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerEvent {
    Picked(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) layer: Layer,
    pub(crate) current: Option<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Category(Category),
    Code,
}

pub(crate) struct ActionPicker {
    search: Entity<InputState>,
    code: Entity<InputState>,
    query: SharedString,
    entries: Vec<Entry>,
    open: Option<Group>,
    target: Option<Target>,
    none_label: &'static str,
    capabilities: Option<Capabilities>,
    profile_names: Vec<SharedString>,
    listening: Option<Listening>,
    focus: FocusHandle,
    code_problem: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy)]
struct Listening {
    modifiers: Modifiers,
    caps_lock: bool,
}

impl EventEmitter<PickerEvent> for ActionPicker {}

impl ActionPicker {
    pub(crate) fn new(
        none_label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search for an action"));
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
            open: Some(Group::Category(Category::Basic)),
            target: None,
            none_label,
            capabilities: None,
            profile_names: Vec::new(),
            listening: None,
            focus: cx.focus_handle(),
            code_problem: None,
            _subscriptions: subscriptions,
        }
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

    pub(crate) fn set_target(&mut self, target: Option<Target>, cx: &mut Context<Self>) {
        if self.target == target {
            return;
        }
        let shown = target.and_then(|target| target.current);
        if shown != self.target.and_then(|target| target.current)
            && let Some(entry) = self
                .entries
                .iter()
                .find(|entry| Some(entry.action) == shown)
        {
            self.open = Some(Group::Category(entry.category));
        }
        self.target = target;
        if target.is_none() {
            self.listening = None;
        }
        cx.notify();
    }

    fn toggle_group(&mut self, group: Group, cx: &mut Context<Self>) {
        self.open = (self.open != Some(group)).then_some(group);
        cx.notify();
    }

    fn pick(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target else {
            return;
        };
        if let Some(reason) = catalog::refusal(action, target.layer) {
            window.push_notification(Notification::warning(reason), cx);
            return;
        }
        if let Some(capabilities) = &self.capabilities
            && let Err(invalid) = action.validate(capabilities)
        {
            window.push_notification(Notification::error(invalid.to_string()), cx);
            return;
        }
        self.listening = None;
        cx.emit(PickerEvent::Picked(action));
        cx.notify();
    }

    fn toggle_listening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.listening.take().is_none() && self.target.is_some() {
            self.listening = Some(Listening {
                modifiers: window.modifiers(),
                caps_lock: window.capslock().on,
            });
            self.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn captured_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.listening.is_none() || !self.focus.is_focused(window) {
            return;
        }
        cx.stop_propagation();
        let key = event.keystroke.key.as_str();
        if let Some(code) = quick_bind::usage_of_key(key) {
            self.pick(Action::Keyboard(code), window, cx);
        } else {
            self.listening = None;
            cx.notify();
            window.push_notification(
                Notification::warning(format!("Can't quick bind {key}; pick it from the list")),
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
        if let Some(capabilities) = &self.capabilities
            && let Err(invalid) = action.validate(capabilities)
        {
            self.code_problem = Some(capitalized(&invalid.to_string()).into());
            cx.notify();
            return;
        }
        self.code_problem = None;
        self.pick(action, window, cx);
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let listening = self.listening.is_some();
        let idle = self.target.is_none();
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(
                Button::new("quick-bind")
                    .ghost()
                    .icon(Catalog::Keyboard)
                    .label(if listening {
                        "Press a key…"
                    } else {
                        "Quick bind"
                    })
                    .selected(listening)
                    .disabled(idle)
                    .tooltip("Binds the next key you press")
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_listening(window, cx))),
            )
            .child(
                Button::new("pick-none")
                    .ghost()
                    .icon(IconName::Ban)
                    .label(self.none_label)
                    .selected(
                        self.target
                            .is_some_and(|target| target.current == Some(Action::None)),
                    )
                    .disabled(idle)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick(Action::None, window, cx)),
                    ),
            )
    }

    fn groups(&self, cx: &mut Context<Self>) -> Div {
        let mut list = v_flex();
        for category in Category::ALL {
            let group = Group::Category(category);
            let open = self.open == Some(group);
            list = list.child(group_header(
                group,
                category_icon(category),
                category.title(),
                open,
                cx,
            ));
            if open {
                let entries: Vec<Entry> = self
                    .entries
                    .iter()
                    .filter(|entry| entry.category == category)
                    .copied()
                    .collect();
                list = list.child(self.grid(entries, false, cx));
            }
        }
        let open = self.open == Some(Group::Code);
        list.child(group_header(
            Group::Code,
            Catalog::SquareTerminal,
            "HID usage",
            open,
            cx,
        ))
        .when(open, |this| this.child(self.code_row(cx)))
    }

    fn search_results(&self, cx: &mut Context<Self>) -> Div {
        let found = catalog::search(&self.entries, &self.query);
        if found.is_empty() {
            let special = !self.query.is_ascii();
            return v_flex()
                .gap_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No results")
                .when(special, |this| {
                    this.child("Special characters depend on the computer's language")
                });
        }
        self.grid(found, true, cx)
    }

    fn grid(&self, entries: Vec<Entry>, searching: bool, cx: &mut Context<Self>) -> Div {
        let tiles: Vec<Button> = entries
            .into_iter()
            .map(|entry| self.action_button(entry, searching, cx))
            .collect();
        h_flex().gap_2().flex_wrap().pt_1().pb_4().children(tiles)
    }

    fn action_button(&self, entry: Entry, searching: bool, cx: &mut Context<Self>) -> Button {
        let action = entry.action;
        let refusal = self
            .target
            .and_then(|target| catalog::refusal(action, target.layer));
        let keyish = matches!(
            entry.category,
            Category::Basic
                | Category::Extended
                | Category::International
                | Category::OtherKeys
                | Category::Gamepad
        );
        let label = if keyish {
            usage::legend(action)
        } else {
            usage::name(action)
        };
        let mut tooltip = usage::name(action).into_owned();
        if let Action::Function(Function::Profile(slot)) = action
            && let Some(name) = self.profile_names.get(usize::from(slot.get()))
        {
            tooltip = format!("{tooltip}: {name}");
        }
        if searching {
            tooltip = format!("{tooltip} · {}", entry.category.title());
        }
        if let Some(reason) = refusal {
            reason.clone_into(&mut tooltip);
        }
        let current = self
            .target
            .is_some_and(|target| target.current == Some(action));
        let accent = cx.theme().primary;
        Button::new(SharedString::from(format!("pick-{action:?}")))
            .small()
            .map(|button| {
                if keyish {
                    button.min_w(px(TILE_SIZE)).h(px(TILE_SIZE))
                } else {
                    button.h(px(WIDE_TILE_HEIGHT))
                }
            })
            .label(SharedString::from(label))
            .tooltip(tooltip)
            .selected(current)
            .when(current, |button| button.border_color(accent))
            .disabled(self.target.is_none() || refusal.is_some())
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
                Button::new("bind-keyboard-code")
                    .small()
                    .label("Keyboard")
                    .disabled(idle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.bind_code(false, window, cx);
                    })),
            )
            .child(
                Button::new("bind-consumer-code")
                    .small()
                    .label("Consumer")
                    .disabled(idle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.bind_code(true, window, cx);
                    })),
            )
            .when_some(self.code_problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(danger).child(problem))
            })
    }
}

impl Render for ActionPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let searching = !self.query.trim().is_empty();
        v_flex()
            .id("action-picker")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::captured_key))
            .on_modifiers_changed(cx.listener(Self::captured_modifiers))
            .w_full()
            .gap_4()
            .child(self.toolbar(cx))
            .child(if searching {
                self.search_results(cx)
            } else {
                self.groups(cx)
            })
    }
}

fn group_header(
    group: Group,
    icon: Catalog,
    title: &'static str,
    open: bool,
    cx: &mut Context<ActionPicker>,
) -> Stateful<Div> {
    let theme = cx.theme();
    let (badge, muted, radius) = (theme.accent, theme.muted_foreground, theme.radius);
    h_flex()
        .id(SharedString::from(format!("group-{title}")))
        .w_full()
        .h(px(GROUP_HEIGHT))
        .gap_3()
        .items_center()
        .cursor_pointer()
        .child(
            h_flex()
                .flex_none()
                .size(px(GROUP_ICON_SIZE))
                .items_center()
                .justify_center()
                .rounded(radius)
                .bg(badge)
                .child(Icon::new(icon).small()),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(title),
        )
        .child(
            div().text_color(muted).child(
                Icon::new(if open {
                    Catalog::ChevronUp
                } else {
                    Catalog::ChevronDown
                })
                .small(),
            ),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle_group(group, cx)))
}

const fn category_icon(category: Category) -> Catalog {
    match category {
        Category::Basic => Catalog::CaseSensitive,
        Category::Extended => Catalog::Keyboard,
        Category::International => Catalog::Globe,
        Category::OtherKeys => Catalog::Ellipsis,
        Category::Functions => Catalog::Zap,
        Category::Layers => Catalog::Layers,
        Category::Profiles => Catalog::UserRound,
        Category::Media => Catalog::Music,
        Category::Applications => Catalog::Monitor,
        Category::System => Catalog::Power,
        Category::Mouse => Catalog::Mouse,
        Category::Gamepad => Catalog::Gamepad2,
    }
}

fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}
