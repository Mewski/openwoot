mod edit;
mod editor;
mod library;
mod recording;
mod step;
mod text;

use std::time::{Duration, Instant};

use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyUpEvent, Keystroke, ModifiersChangedEvent, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Task, Window,
    component::{
        WindowExt as _, h_flex,
        input::{InputEvent, InputState},
        notification::Notification,
        v_flex,
    },
    div, px,
};
use openwoot_protocol::{
    configuration::{Invalid, Validate as _},
    endpoints::GetMacro,
    keys::Action,
    macros::{MAX_STEPS, Macro, Mode, Step},
};

use self::{
    recording::{Recording, Timing},
    step::{Kind, Part},
};
use super::{
    Page,
    remap::{ActionPicker, PickerEvent, Target, capitalized, modifier_usages, usage_of_key},
};
use crate::{
    device::Failure,
    panel,
    store::{Store, StoreEvent},
};

const LIBRARY_WIDTH: f32 = 370.0;
const COLUMN_GAP: f32 = 16.0;
const MACROS_HELP: &str =
    "Build key sequences, record them from your keyboard and bind them to keys in Remap.";
const FULL: &str = "A macro holds up to 256 steps";
const TOO_LONG: &str = "The macro is too long for the keyboard, remove steps or shorten texts";
const MODES: [Mode; 3] = [Mode::Once, Mode::WhileHeld, Mode::Toggle];

pub(crate) struct MacrosPage {
    store: Entity<Store>,
    selected: Option<u8>,
    content: Option<(u8, Macro)>,
    revision: u64,
    chosen: Option<usize>,
    target: KeyTarget,
    field: Option<Field>,
    value: Entity<InputState>,
    name: Entity<InputState>,
    picker: Entity<ActionPicker>,
    recorder: Option<Recorder>,
    timing: Timing,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyTarget {
    Append,
    Step(usize),
    New(Kind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Field {
    step: usize,
    part: Part,
}

struct Recorder {
    recording: Recording,
    started: Instant,
    at: usize,
    room: usize,
    caps_lock: bool,
    skipped: Vec<String>,
    _interceptor: Subscription,
    _activation: Subscription,
}

impl MacrosPage {
    pub(crate) fn new(store: Entity<Store>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| ActionPicker::new(&store, window, cx).for_steps());
        let value = cx.new(|cx| InputState::new(window, cx));
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Macro name"));
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::Connected | StoreEvent::Reloaded => this.load(window, cx),
                StoreEvent::Disconnected => this.forget(cx),
                StoreEvent::ProfileChanged | StoreEvent::Failed(_) => {}
            }),
            cx.subscribe_in(
                &picker,
                window,
                |this, _, event: &PickerEvent, window, cx| match event {
                    PickerEvent::Picked(action) => this.picked(*action, window, cx),
                    PickerEvent::Cancelled => this.retarget(KeyTarget::Append, cx),
                },
            ),
            cx.subscribe_in(
                &value,
                window,
                |this, state, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        let typed = state.read(cx).value();
                        this.commit_field(&typed, window, cx);
                    }
                    InputEvent::Change | InputEvent::Focus => {}
                },
            ),
            cx.subscribe_in(
                &name,
                window,
                |this, state, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } | InputEvent::Blur => {
                        let typed = state.read(cx).value();
                        this.rename(&typed, window, cx);
                    }
                    InputEvent::Change | InputEvent::Focus => {}
                },
            ),
        ];
        let mut page = Self {
            store,
            selected: None,
            content: None,
            revision: 0,
            chosen: None,
            target: KeyTarget::Append,
            field: None,
            value,
            name,
            picker,
            recorder: None,
            timing: Timing::default(),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        page.load(window, cx);
        page
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return;
        };
        let selected = self
            .selected
            .filter(|index| keyboard.macros.get(*index).is_some())
            .or_else(|| keyboard.macros.iter().next().map(|(index, _)| index));
        self.show(selected, window, cx);
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.recorder = None;
        self.content = None;
        self.field = None;
        self.target = KeyTarget::Append;
        self.revision += 1;
        cx.notify();
    }

    fn select(&mut self, index: u8, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_recording(window, cx);
        if self.selected != Some(index) {
            self.show(Some(index), window, cx);
        }
    }

    fn show(&mut self, index: Option<u8>, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != index {
            self.chosen = None;
        }
        self.field = None;
        self.target = KeyTarget::Append;
        self.picker.update(cx, ActionPicker::stop_listening);
        self.selected = index;
        self.revision += 1;
        self.sync_name(window, cx);
        self.sync_picker(cx);
        cx.notify();
        let Some(index) = index else {
            self.content = None;
            return;
        };
        let fetch = self
            .store
            .update(cx, |store, cx| store.fetch::<GetMacro, _>(index, cx));
        let revision = self.revision;
        cx.spawn_in(window, async move |this, cx| {
            let loaded = Box::pin(fetch).await;
            this.update_in(cx, |page, window, cx| {
                if page.revision != revision {
                    return;
                }
                page.content = loaded.ok().flatten().map(|value| (index, value));
                page.sync_name(window, cx);
                page.sync_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn loaded(&self) -> Option<&Macro> {
        self.content
            .as_ref()
            .filter(|(index, _)| Some(*index) == self.selected)
            .map(|(_, value)| value)
    }

    fn step(&self, index: usize) -> Option<&Step> {
        self.loaded().and_then(|value| value.steps.get(index))
    }

    fn insertion(&self) -> usize {
        let len = self.loaded().map_or(0, |value| value.steps.len());
        self.chosen.map_or(len, |chosen| chosen + 1).min(len)
    }

    fn edit(
        &mut self,
        change: impl FnOnce(&mut Macro),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((index, before)) = self
            .content
            .clone()
            .filter(|(index, _)| Some(*index) == self.selected)
        else {
            return false;
        };
        let mut after = before.clone();
        change(&mut after);
        if after == before {
            return false;
        }
        if let Some(problem) = self.problem(&after, cx) {
            window.push_notification(Notification::warning(problem), cx);
            return false;
        }
        self.content = Some((index, after.clone()));
        self.revision += 1;
        let applied = self.store.update(cx, |store, cx| {
            store.set_macro(index, Some(&before), Some(after), cx)
        });
        Self::reload_on_failure(applied, window, cx);
        self.sync_picker(cx);
        cx.notify();
        true
    }

    fn reload_on_failure(
        applied: Task<Result<(), Failure>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            if applied.await.is_err() {
                this.update_in(cx, |page, window, cx| {
                    page.revision += 1;
                    page.content = None;
                    page.load(window, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn problem(&self, value: &Macro, cx: &App) -> Option<String> {
        let keyboard = self.store.read(cx).keyboard()?;
        match value.validate(&keyboard.capabilities) {
            Ok(()) => None,
            Err(Invalid::MacroTooLong(_)) => Some(TOO_LONG.to_owned()),
            Err(invalid) => Some(capitalized(&invalid.to_string())),
        }
    }

    fn sync_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .loaded()
            .map(|value| value.name.as_str().to_owned())
            .or_else(|| {
                let index = self.selected?;
                let keyboard = self.store.read(cx).keyboard()?;
                keyboard.macros.name(index).map(str::to_owned)
            })
            .unwrap_or_default();
        if self.name.read(cx).value().as_ref() != name {
            self.name
                .update(cx, |input, cx| input.set_value(name, window, cx));
        }
    }

    fn sync_picker(&self, cx: &mut Context<Self>) {
        let current = match self.target {
            KeyTarget::Step(index) => self.step(index).and_then(step::action),
            KeyTarget::Append | KeyTarget::New(_) => None,
        };
        let target = self.loaded().map(|_| Target {
            current: current.unwrap_or_default(),
        });
        self.picker
            .update(cx, |picker, cx| picker.set_target(target, cx));
    }

    fn retarget(&mut self, target: KeyTarget, cx: &mut Context<Self>) {
        self.target = target;
        if target == KeyTarget::Append {
            self.picker.update(cx, ActionPicker::stop_listening);
        }
        self.sync_picker(cx);
        cx.notify();
    }

    fn listen_for_key(&mut self, target: KeyTarget, window: &mut Window, cx: &mut Context<Self>) {
        self.field = None;
        self.retarget(target, cx);
        self.picker
            .update(cx, |picker, cx| picker.listen(window, cx));
    }

    fn rename(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        let renamed =
            edit::named(typed).is_some_and(|name| self.edit(|value| value.name = name, window, cx));
        if !renamed {
            self.sync_name(window, cx);
        }
    }

    fn set_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(|value| value.mode = mode, window, cx);
    }

    fn insert_steps(
        &mut self,
        at: usize,
        steps: &[Step],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let mut inserted = None;
        let applied = self.edit(
            |value| inserted = Some(edit::insert(&mut value.steps, at, steps)),
            window,
            cx,
        );
        let Some(inserted) = inserted else {
            return 0;
        };
        if inserted < steps.len() {
            window.push_notification(Notification::warning(FULL), cx);
        }
        if !applied || inserted == 0 {
            return 0;
        }
        self.chosen = Some(at + inserted - 1);
        inserted
    }

    fn add_step(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        if kind.is_key() {
            self.listen_for_key(KeyTarget::New(kind), window, cx);
            return;
        }
        let Some(step) = kind.blank() else {
            return;
        };
        let at = self.insertion();
        if self.insert_steps(at, &[step], window, cx) > 0
            && let Some(part) = kind.parts().first()
        {
            self.begin_field(
                Field {
                    step: at,
                    part: *part,
                },
                window,
                cx,
            );
        }
    }

    fn picked(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match self.target {
            KeyTarget::Append => {
                let at = self.insertion();
                self.insert_steps(at, &[Step::Tap(action)], window, cx);
            }
            KeyTarget::Step(index) => self.set_action(index, action, window, cx),
            KeyTarget::New(kind) => {
                if let Some(step) = kind.with_action(action) {
                    let at = self.insertion();
                    self.insert_steps(at, &[step], window, cx);
                }
            }
        }
        self.retarget(KeyTarget::Append, cx);
    }

    fn append_dropped(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.insertion();
        self.insert_steps(at, &[Step::Tap(action)], window, cx);
        self.retarget(KeyTarget::Append, cx);
    }

    fn choose_key(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.target == KeyTarget::Step(index) {
            self.retarget(KeyTarget::Append, cx);
            return;
        }
        self.chosen = Some(index);
        self.listen_for_key(KeyTarget::Step(index), window, cx);
    }

    fn set_action(
        &mut self,
        index: usize,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.edit(
            |value| {
                if let Some(step) = value.steps.get_mut(index)
                    && let Some(changed) = Kind::of(step).with_action(action)
                {
                    *step = changed;
                }
            },
            window,
            cx,
        );
    }

    fn set_kind(&mut self, index: usize, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        self.edit(
            |value| {
                if let Some(step) = value.steps.get_mut(index)
                    && let Some(changed) =
                        step::action(step).and_then(|action| kind.with_action(action))
                {
                    *step = changed;
                }
            },
            window,
            cx,
        );
    }

    fn remove_step(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let removed = self.edit(
            |value| {
                edit::remove(&mut value.steps, index);
            },
            window,
            cx,
        );
        if removed {
            self.chosen = match self.chosen {
                Some(chosen) if chosen > index => Some(chosen - 1),
                Some(chosen) if chosen == index => index.checked_sub(1),
                chosen => chosen,
            };
            self.field = None;
            self.retarget(KeyTarget::Append, cx);
        }
    }

    fn move_step(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        let moved = self.edit(
            |value| {
                edit::reorder(&mut value.steps, from, to);
            },
            window,
            cx,
        );
        if moved {
            self.chosen = Some(to);
            self.field = None;
            self.retarget(KeyTarget::Append, cx);
        }
    }

    fn choose_step(&mut self, index: usize, cx: &mut Context<Self>) {
        self.chosen = if self.chosen == Some(index) {
            None
        } else {
            Some(index)
        };
        cx.notify();
    }

    fn begin_field(&mut self, field: Field, window: &mut Window, cx: &mut Context<Self>) {
        if self.field.is_some_and(|editing| editing != field) {
            let typed = self.value.read(cx).value();
            self.commit_field(&typed, window, cx);
        }
        let Some(step) = self.step(field.step) else {
            return;
        };
        let shown = field.part.shown(step);
        self.field = Some(field);
        self.chosen = Some(field.step);
        self.value.update(cx, |input, cx| {
            input.set_value(shown, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn commit_field(&mut self, typed: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.field.take() else {
            return;
        };
        cx.notify();
        let Some(step) = self.step(field.step).cloned() else {
            return;
        };
        match field.part.applied(&step, typed) {
            Ok(changed) => {
                self.edit(
                    |value| {
                        if let Some(slot) = value.steps.get_mut(field.step) {
                            *slot = changed;
                        }
                    },
                    window,
                    cx,
                );
            }
            Err(problem) => window.push_notification(Notification::warning(problem), cx),
        }
    }

    fn cancel_field(&mut self, cx: &mut Context<Self>) {
        if self.field.take().is_some() {
            cx.notify();
        }
    }

    fn start_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(len) = self.loaded().map(|value| value.steps.len()) else {
            return;
        };
        let room = MAX_STEPS.saturating_sub(len);
        if room == 0 {
            window.push_notification(Notification::warning(FULL), cx);
            return;
        }
        self.field = None;
        self.retarget(KeyTarget::Append, cx);
        self.focus.focus(window, cx);
        let page = cx.entity().downgrade();
        let interceptor = cx.intercept_keystrokes(move |event, window, cx| {
            let consumed = page
                .update(cx, |page, cx| {
                    page.intercepted(&event.keystroke, window, cx)
                })
                .unwrap_or(false);
            if consumed {
                cx.stop_propagation();
            }
        });
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.stop_recording(window, cx);
            }
        });
        self.recorder = Some(Recorder {
            recording: Recording::with_modifiers(
                &modifier_usages(window.modifiers()).collect::<Vec<_>>(),
            ),
            started: Instant::now(),
            at: self.insertion(),
            room,
            caps_lock: window.capslock().on,
            skipped: Vec::new(),
            _interceptor: interceptor,
            _activation: activation,
        });
        cx.notify();
    }

    fn stop_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(recorder) = self.recorder.take() else {
            return;
        };
        let mut steps = recorder.recording.steps(self.timing);
        steps.truncate(recorder.room);
        if !steps.is_empty() {
            self.insert_steps(recorder.at, &steps, window, cx);
        }
        cx.notify();
    }

    fn recorded(&self) -> Option<(usize, Vec<Step>)> {
        let recorder = self.recorder.as_ref()?;
        let mut steps = recorder.recording.steps(self.timing);
        steps.truncate(recorder.room);
        Some((recorder.at, steps))
    }

    fn elapsed(&self) -> Duration {
        self.recorder
            .as_ref()
            .map_or(Duration::ZERO, |recorder| recorder.started.elapsed())
    }

    fn intercepted(
        &mut self,
        keystroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.recorder.is_none() {
            return false;
        }
        if !self.focus.contains_focused(window, cx) {
            self.stop_recording(window, cx);
            return false;
        }
        if keystroke.is_modifier_key() || keystroke.key == "capslock" {
            return false;
        }
        let at = self.elapsed();
        let Some(recorder) = &mut self.recorder else {
            return false;
        };
        match usage_of_key(&keystroke.key) {
            Some(usage) => recorder.recording.press(usage, at),
            None if !recorder.skipped.contains(&keystroke.key) => {
                recorder.skipped.push(keystroke.key.clone());
                let message = format!("{} can't be recorded, add it from the list", keystroke.key);
                window.push_notification(Notification::warning(message), cx);
            }
            None => {}
        }
        self.keep_room(window, cx);
        true
    }

    fn key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.elapsed();
        let Some(recorder) = &mut self.recorder else {
            return;
        };
        if let Some(usage) = usage_of_key(&event.keystroke.key) {
            recorder.recording.release(usage, at);
            self.keep_room(window, cx);
        }
    }

    fn modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.elapsed();
        let Some(recorder) = &mut self.recorder else {
            return;
        };
        let held: Vec<u8> = modifier_usages(event.modifiers).collect();
        recorder.recording.hold_modifiers(&held, at);
        if event.capslock.on != recorder.caps_lock {
            recorder.caps_lock = event.capslock.on;
            recorder.recording.toggle_caps_lock(at);
        }
        self.keep_room(window, cx);
    }

    fn keep_room(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let full = self
            .recorder
            .as_ref()
            .is_some_and(|recorder| recorder.recording.steps(self.timing).len() >= recorder.room);
        if full {
            self.stop_recording(window, cx);
            window.push_notification(Notification::warning(FULL), cx);
        }
        cx.notify();
    }

    fn set_timing(&mut self, timing: Timing, cx: &mut Context<Self>) {
        self.timing = timing;
        cx.notify();
    }
}

impl Render for MacrosPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(keyboard) = self.store.read(cx).keyboard().cloned() else {
            return panel::page();
        };
        let editor = self.editor_card(&keyboard, window, cx);
        let keys = self.keys_card(cx);
        let body = h_flex()
            .w_full()
            .items_start()
            .gap(px(COLUMN_GAP))
            .child(
                div()
                    .flex_none()
                    .w(px(LIBRARY_WIDTH))
                    .child(self.library_card(&keyboard.macros, cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(COLUMN_GAP))
                    .child(editor)
                    .children(keys),
            );
        panel::page_with_heading(
            "macros-body",
            panel::heading(Page::Macros.title()).help(MACROS_HELP),
            body,
        )
        .track_focus(&self.focus)
        .on_key_up(cx.listener(Self::key_up))
        .on_modifiers_changed(cx.listener(Self::modifiers_changed))
    }
}

fn step_count(steps: usize) -> SharedString {
    match steps {
        1 => "1 step".into(),
        count => format!("{count} steps").into(),
    }
}

const fn mode_title(mode: Mode) -> &'static str {
    match mode {
        Mode::Once => "Play once",
        Mode::WhileHeld => "Repeat while held",
        Mode::Toggle => "Toggle repeat",
    }
}

const fn mode_detail(mode: Mode) -> &'static str {
    match mode {
        Mode::Once => "Plays all steps once on each press",
        Mode::WhileHeld => "Repeats the steps while the key is held and finishes the last pass",
        Mode::Toggle => "A press starts repeating the steps and the next press stops them",
    }
}
