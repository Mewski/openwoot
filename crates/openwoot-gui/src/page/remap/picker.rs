use gpui_kit::{
    AppContext as _, Context, Div, Entity, EventEmitter, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, Modifiers, ModifiersChangedEvent,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window,
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

const SPECIAL_CHARACTERS: &str = "Special characters can't be remapped. The keyboard itself has no \
     language: it sends standard key codes, which the computer turns into characters depending on \
     the language selected in the operating system.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PickerEvent {
    Picked(Action),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) layer: Layer,
    pub(crate) current: Option<Action>,
}

pub(crate) struct ActionPicker {
    search: Entity<InputState>,
    code: Entity<InputState>,
    query: SharedString,
    entries: Vec<Entry>,
    category: Category,
    target: Option<Target>,
    idle_hint: &'static str,
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
        idle_hint: &'static str,
        none_label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search for a character"));
        let code = cx.new(|cx| InputState::new(window, cx).placeholder("Code, e.g. 0x65"));
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
            category: Category::Basic,
            target: None,
            idle_hint,
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
        if self.target != target {
            self.target = target;
            if target.is_none() {
                self.listening = None;
            }
            cx.notify();
        }
    }

    fn pick(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target else {
            window.push_notification(Notification::warning(self.idle_hint), cx);
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

    fn listen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.target.is_none() {
            window.push_notification(Notification::warning(self.idle_hint), cx);
            return;
        }
        self.listening = Some(Listening {
            modifiers: window.modifiers(),
            caps_lock: window.capslock().on,
        });
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn stop_listening(&mut self, cx: &mut Context<Self>) {
        self.listening = None;
        cx.notify();
    }

    fn captured_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.listening.is_none() {
            return;
        }
        cx.stop_propagation();
        let key = event.keystroke.key.as_str();
        if let Some(code) = quick_bind::usage_of_key(key) {
            self.pick(Action::Keyboard(code), window, cx);
        } else {
            self.stop_listening(cx);
            window.push_notification(
                Notification::warning(format!(
                    "The {key} key has no key code to bind by pressing it; pick it from the list"
                )),
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
            self.code_problem = Some(
                if consumer {
                    "Enter a consumer usage as a hexadecimal number from 0x0001 to 0x02a2"
                } else {
                    "Enter a keyboard usage as a hexadecimal number from 0x04 to 0xe7"
                }
                .into(),
            );
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
        h_flex()
            .gap_2()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.search)
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small()),
                ),
            )
            .child(
                Button::new("quick-bind")
                    .small()
                    .outline()
                    .icon(Catalog::Keyboard)
                    .label("Quick bind")
                    .selected(listening)
                    .tooltip("Press a key on any keyboard to bind its key code")
                    .on_click(cx.listener(|this, _, window, cx| {
                        if this.listening.is_some() {
                            this.stop_listening(cx);
                        } else {
                            this.listen(window, cx);
                        }
                    })),
            )
            .child(
                Button::new("pick-none")
                    .small()
                    .outline()
                    .icon(IconName::Ban)
                    .label(self.none_label)
                    .selected(
                        self.target
                            .is_some_and(|target| target.current == Some(Action::None)),
                    )
                    .on_click(
                        cx.listener(|this, _, window, cx| this.pick(Action::None, window, cx)),
                    ),
            )
    }

    fn banner(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        if self.listening.is_some() {
            return h_flex().child(
                div()
                    .id("quick-bind-prompt")
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(Self::captured_key))
                    .on_modifiers_changed(cx.listener(Self::captured_modifiers))
                    .on_click(cx.listener(|this, _, window, cx| this.focus.focus(window, cx)))
                    .flex_1()
                    .p_3()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.primary)
                    .bg(theme.primary.opacity(0.08))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Icon::new(Catalog::Keyboard))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Press any key to bind"),
                                    )
                                    .child(
                                        div().text_xs().text_color(theme.muted_foreground).child(
                                            "Keys are read as on a US layout; modifiers bind \
                                                 their left-hand key.",
                                        ),
                                    ),
                            )
                            .child(
                                Button::new("quick-bind-cancel")
                                    .small()
                                    .ghost()
                                    .label("Cancel")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.stop_listening(cx)),
                                    ),
                            ),
                    ),
            );
        }
        let hint: SharedString = match self.target {
            None => self.idle_hint.into(),
            Some(Target {
                current: Some(action),
                ..
            }) => format!("Current: {}", usage::name(action)).into(),
            Some(_) => "The selected keys have different actions".into(),
        };
        h_flex()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(hint)
    }

    fn categories(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .gap_1()
            .flex_wrap()
            .children(Category::ALL.map(|category| {
                Button::new(SharedString::from(format!("category-{category:?}")))
                    .small()
                    .when(category == self.category, Button::primary)
                    .when(category != self.category, Button::ghost)
                    .label(category.title())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.category = category;
                        cx.notify();
                    }))
            }))
    }

    fn results(&self, cx: &mut Context<Self>) -> Div {
        let searching = !self.query.trim().is_empty();
        let shown: Vec<Entry> = if searching {
            catalog::search(&self.entries, &self.query)
        } else {
            self.entries
                .iter()
                .filter(|entry| entry.category == self.category)
                .copied()
                .collect()
        };
        if shown.is_empty() {
            let special = !self.query.is_ascii();
            return v_flex()
                .gap_1()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("No results found")
                .when(special, |this| this.child(SPECIAL_CHARACTERS));
        }
        let buttons: Vec<Button> = shown
            .into_iter()
            .map(|entry| self.action_button(entry, searching, cx))
            .collect();
        h_flex().gap_1().flex_wrap().children(buttons)
    }

    fn action_button(&self, entry: Entry, searching: bool, cx: &mut Context<Self>) -> Button {
        let action = entry.action;
        let refusal = self
            .target
            .and_then(|target| catalog::refusal(action, target.layer));
        let label = match entry.category {
            Category::Basic
            | Category::Extended
            | Category::International
            | Category::OtherKeys
            | Category::Gamepad => usage::legend(action),
            _ => usage::name(action),
        };
        let mut tooltip = usage::name(action).into_owned();
        if let Action::Function(Function::Profile(slot)) = action {
            match self.profile_names.get(usize::from(slot.get())) {
                Some(name) => tooltip = format!("{tooltip}: {name}"),
                None => tooltip.push_str(" (this profile does not exist yet)"),
            }
        }
        if searching {
            tooltip = format!("{tooltip} · {}", entry.category.title());
        }
        if let Some(reason) = refusal {
            reason.clone_into(&mut tooltip);
        }
        Button::new(SharedString::from(format!("pick-{action:?}")))
            .small()
            .outline()
            .label(SharedString::from(label))
            .tooltip(tooltip)
            .selected(
                self.target
                    .is_some_and(|target| target.current == Some(action)),
            )
            .disabled(self.target.is_none() || refusal.is_some())
            .on_click(cx.listener(move |this, _, window, cx| this.pick(action, window, cx)))
    }

    fn code_row(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let idle = self.target.is_none();
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Other HID usage"),
                    )
                    .child(div().w_40().child(Input::new(&self.code).small()))
                    .child(
                        Button::new("bind-keyboard-code")
                            .small()
                            .ghost()
                            .label("Bind keyboard usage")
                            .disabled(idle)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.bind_code(false, window, cx);
                            })),
                    )
                    .child(
                        Button::new("bind-consumer-code")
                            .small()
                            .ghost()
                            .label("Bind consumer usage")
                            .disabled(idle)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.bind_code(true, window, cx);
                            })),
                    ),
            )
            .when_some(self.code_problem.clone(), |this, problem| {
                this.child(div().text_xs().text_color(theme.danger).child(problem))
            })
    }
}

impl Render for ActionPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let searching = !self.query.trim().is_empty();
        v_flex()
            .w_full()
            .gap_3()
            .child(self.toolbar(cx))
            .child(self.banner(cx))
            .when(!searching, |this| this.child(self.categories(cx)))
            .child(self.results(cx))
            .child(self.code_row(cx))
    }
}

fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}
