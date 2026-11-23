use heapless::Vec;
use openwoot_protocol::{
    gamepad::{self, Control},
    keys::{Action, Function, Layer},
};

use super::{
    Actuation, Advanced, Behaviour, Call, Effects, Entry, Functions, Gamepad, GamepadReport, Key,
    Latch, NOTICE_CAPACITY, Notice, Output, RapidTrigger, RapidTriggerOverride, SPECIALS,
    SlotChange, Special, State, Thresholds, Trigger, Tuning, Zone, follows_travel_action,
    record::Record,
    special::{Request, Requests},
};
use crate::{
    keys::Pending,
    layout::{LAYERS, Layers, Profiles},
    lighting::{Indicator, KeyEvent},
    usb::Mode,
};

const CAPS_LOCK: u8 = 0x39;
const SCROLL_LOCK: u8 = 0x47;
const NUM_LOCK: u8 = 0x53;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switch {
    pub minimum: u16,
    pub travel_limit: u16,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "the report format, key reports, gamepad detection and the gamepad option are independent switches"
)]
#[derive(Debug)]
pub struct Keys<F: Functions, const ROWS: usize, const COLUMNS: usize> {
    records: [[Record; COLUMNS]; ROWS],
    tuning: Tuning,
    advanced: Advanced,
    pending: Pending,
    layers: Layers,
    profiles: Profiles,
    requests: Requests,
    held: Requests,
    functions: F,
    output: Output,
    gamepad: Gamepad,
    gamepad_settings: Option<gamepad::Settings>,
    gamepad_exclusive: bool,
    mode: Mode,
    boot_format: bool,
    key_reports: bool,
    reporting: bool,
    gamepad_detection: bool,
    xinput_player: Option<u8>,
    elapsed_ms: u16,
}

impl<F: Functions, const ROWS: usize, const COLUMNS: usize> Keys<F, ROWS, COLUMNS> {
    #[must_use]
    pub fn new(functions: F) -> Self {
        const { assert!(ROWS <= 8 && COLUMNS <= 32 && ROWS * COLUMNS < 256) };
        Self {
            records: [[Record::EMPTY; COLUMNS]; ROWS],
            tuning: Tuning::default(),
            advanced: Advanced::new(),
            pending: Pending::new(),
            layers: Layers::new(),
            profiles: Profiles::default(),
            requests: Requests::default(),
            held: Requests::default(),
            functions,
            output: Output::default(),
            gamepad: Gamepad::new(),
            gamepad_settings: None,
            gamepad_exclusive: false,
            mode: Mode::NoGamepad,
            boot_format: false,
            key_reports: true,
            reporting: true,
            gamepad_detection: false,
            xinput_player: None,
            elapsed_ms: 0,
        }
    }

    #[must_use]
    pub const fn functions(&self) -> &F {
        &self.functions
    }

    #[must_use]
    pub const fn tuning(&self) -> &Tuning {
        &self.tuning
    }

    #[must_use]
    pub const fn advanced(&self) -> &Advanced {
        &self.advanced
    }

    #[must_use]
    pub const fn layers(&self) -> &Layers {
        &self.layers
    }

    #[must_use]
    pub const fn profiles(&self) -> &Profiles {
        &self.profiles
    }

    #[must_use]
    pub const fn output(&self) -> &Output {
        &self.output
    }

    #[must_use]
    pub const fn gamepad(&self) -> &Gamepad {
        &self.gamepad
    }

    #[must_use]
    pub const fn gamepad_settings(&self) -> Option<&gamepad::Settings> {
        self.gamepad_settings.as_ref()
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    #[must_use]
    pub fn actuation(&self, key: Key) -> Option<Actuation> {
        self.record(key).map(|record| record.actuation)
    }

    #[must_use]
    pub fn holds_events(&self, key: Key) -> bool {
        self.record(key).is_some_and(Record::holds_events)
    }

    #[must_use]
    pub fn minimum(&self, key: Key) -> Option<u16> {
        self.record(key).map(|record| record.minimum)
    }

    #[must_use]
    pub fn threshold(&self, key: Key) -> Option<u16> {
        self.record(key)
            .map(|record| threshold(record, &self.tuning))
    }

    #[must_use]
    pub fn action(&self, key: Key, layer: Layer) -> Action {
        self.record(key)
            .map_or(Action::None, |record| record.action(layer.number()))
    }

    pub fn take_notices(&mut self) -> Vec<Notice, NOTICE_CAPACITY> {
        self.output.take_notices()
    }

    pub const fn take_profile_request(&mut self) -> Option<u8> {
        self.profiles.take_request()
    }

    pub const fn set_profiles(&mut self, count: u8, active: u8) {
        self.profiles.set(count, active);
    }

    pub const fn set_tuning(&mut self, tuning: Tuning) {
        self.tuning = tuning;
    }

    pub const fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub const fn set_boot_format(&mut self, boot_format: bool) {
        self.boot_format = boot_format;
    }

    pub const fn set_key_reports(&mut self, key_reports: bool) {
        self.key_reports = key_reports;
    }

    pub const fn set_gamepad_detection(&mut self, gamepad_detection: bool) {
        self.gamepad_detection = gamepad_detection;
    }

    pub const fn set_xinput_player(&mut self, xinput_player: Option<u8>) {
        self.xinput_player = xinput_player;
    }

    pub const fn set_gamepad_settings(&mut self, settings: Option<gamepad::Settings>) {
        self.gamepad_settings = settings;
    }

    pub const fn set_gamepad_exclusive(&mut self, exclusive: bool) {
        self.gamepad_exclusive = exclusive;
    }

    pub fn set_switch(&mut self, key: Key, switch: Switch) {
        if let Some(record) = self.slot_mut(key) {
            record.present = true;
            record.minimum = switch.minimum;
            record.travel_limit = switch.travel_limit;
        }
    }

    pub fn set_override(
        &mut self,
        key: Key,
        actuation_point: Option<u16>,
        rapid_trigger: RapidTriggerOverride,
    ) {
        if let Some(record) = self.slot_mut(key) {
            record.actuation_point = actuation_point;
            record.rapid_trigger = rapid_trigger;
        }
    }

    pub fn set_action(&mut self, key: Key, layer: Layer, action: Action) {
        let Some(record) = self.slot_mut(key) else {
            return;
        };
        let held_here = record.actuation.is_pressed() && record.latched_layer == layer.number();
        let Some(slot) = record.actions.get_mut(usize::from(layer.number())) else {
            return;
        };
        let replaced = *slot != action;
        *slot = action;
        if replaced && held_here {
            record.ignore_until_release = true;
        }
    }

    pub fn set_gamepad_binding(&mut self, key: Key, control: Option<Control>) {
        if let Some(record) = self.slot_mut(key) {
            record.gamepad = control;
        }
    }

    pub fn set_advanced(&mut self, entries: impl IntoIterator<Item = Entry>) {
        for key in self.advanced.latched_toggles() {
            self.output.notify(Notice::Toggle {
                key,
                latched: false,
                layer_key: false,
            });
        }
        self.advanced.replace(entries);
        self.pending.clear();
        for record in self.records.iter_mut().flatten() {
            record.advanced = [None; LAYERS];
            record.active = [false; LAYERS];
        }
        for index in 0..=u8::MAX {
            let Some(&entry) = self.advanced.entry(index) else {
                break;
            };
            self.bind(entry.key, entry.layer, index);
            if let Some(other) = entry.behaviour.other() {
                self.bind(other, entry.layer, index);
            }
        }
    }

    pub fn reset_keys(&mut self) {
        self.layers.reset();
        self.held.clear();
        self.restart_advanced();
        self.output.notify(Notice::ShowLayer(0));
        let mut lifted: Vec<Key, NOTICE_CAPACITY> = Vec::new();
        for (row, records) in (0..).zip(self.records.iter_mut()) {
            for (column, record) in (0..).zip(records.iter_mut()) {
                record.actuation.reset();
                if core::mem::take(&mut record.disabled) {
                    let _ = lifted.push(Key::new(row, column));
                }
            }
        }
        for key in lifted {
            self.output.notify(Notice::Disabled {
                key,
                disabled: false,
            });
        }
    }

    #[must_use]
    pub fn indicator_keys(&self) -> [(Indicator, Option<Key>); 4] {
        let mut keys = [
            (Indicator::CapsLock, None),
            (Indicator::ScrollLock, None),
            (Indicator::NumLock, None),
            (Indicator::LayerLock, None),
        ];
        let keymaps = (0..LAYERS).flat_map(|layer| {
            (0..)
                .zip(self.records.iter())
                .flat_map(move |(row, records)| {
                    (0..)
                        .zip(records.iter())
                        .filter(|(_, record)| record.present)
                        .map(move |(column, record)| {
                            (
                                record.actions.get(layer).copied().unwrap_or_default(),
                                Key::new(row, column),
                            )
                        })
                })
        });
        let entries = self.advanced.entries().iter().flat_map(|entry| {
            entry
                .behaviour
                .actions()
                .into_iter()
                .map(move |action| (action, entry.key))
        });
        for (action, key) in keymaps.chain(entries) {
            let indicator = match action {
                Action::Keyboard(CAPS_LOCK) => Indicator::CapsLock,
                Action::Keyboard(SCROLL_LOCK) => Indicator::ScrollLock,
                Action::Keyboard(NUM_LOCK) => Indicator::NumLock,
                Action::Layer(Layer::One) => Indicator::LayerLock,
                _ => continue,
            };
            if let Some((_, slot)) = keys.iter_mut().find(|(kind, _)| *kind == indicator)
                && slot.is_none()
            {
                *slot = Some(key);
            }
        }
        keys
    }

    pub fn pass(
        &mut self,
        events: impl IntoIterator<Item = (Key, u16)>,
        elapsed_ms: u16,
        travels: impl Fn(Key) -> u16,
    ) -> &Output {
        self.begin_pass(elapsed_ms);
        for (key, travel) in events {
            self.process_event(key, travel, &travels);
        }
        self.finish_pass()
    }

    pub fn begin_pass(&mut self, elapsed_ms: u16) {
        self.output.clear(self.boot_format);
        self.requests.clear();
        self.elapsed_ms = elapsed_ms;
        if core::mem::replace(&mut self.reporting, self.key_reports) && !self.key_reports {
            self.restart_advanced();
        }
    }

    fn finish_pass(&mut self) -> &Output {
        self.resolve_pairs();
        self.build_gamepad_report();
        if self.key_reports {
            self.dispatch_specials();
        }
        &self.output
    }

    fn restart_advanced(&mut self) {
        for key in self.advanced.latched_toggles() {
            self.output.notify(Notice::Toggle {
                key,
                latched: false,
                layer_key: false,
            });
        }
        self.advanced.restart();
        self.pending.clear();
        for record in self.records.iter_mut().flatten() {
            record.active = [false; LAYERS];
        }
    }

    fn record(&self, key: Key) -> Option<&Record> {
        self.records
            .get(key.row())
            .and_then(|row| row.get(key.column()))
            .filter(|record| record.present)
    }

    fn record_mut(&mut self, key: Key) -> Option<&mut Record> {
        self.slot_mut(key).filter(|record| record.present)
    }

    fn slot_mut(&mut self, key: Key) -> Option<&mut Record> {
        self.records
            .get_mut(key.row())
            .and_then(|row| row.get_mut(key.column()))
    }

    fn bind(&mut self, key: Key, layer: u8, index: u8) {
        if let Some(record) = self.record_mut(key) {
            record.set_active(layer, false);
            if let Some(slot) = record.advanced.get_mut(usize::from(layer)) {
                *slot = Some(index);
            }
            record.actuation.release_for_rebinding();
        }
    }

    fn binding_layer(&self, record: &Record) -> u8 {
        if record.any_active() {
            let latched_toggle = (0u8..).take(LAYERS).find_map(|layer| {
                let index = record.advanced(layer)?;
                let entry = self.advanced.entry(index)?;
                match (entry.behaviour, self.advanced.state(index)?) {
                    (Behaviour::Toggle(_), State::Toggle(toggle)) if toggle.is_latched() => {
                        Some(entry.layer)
                    }
                    _ => None,
                }
            });
            if let Some(layer) = latched_toggle {
                return layer;
            }
        }
        if record.actuation.is_pressed() {
            record.latched_layer
        } else {
            self.layers.active()
        }
    }

    fn current_entry(&self, record: &Record) -> Option<(u8, &Entry)> {
        let index = record.advanced(self.binding_layer(record))?;
        Some((index, self.advanced.entry(index)?))
    }

    fn process_event(&mut self, key: Key, travel: u16, travels: &impl Fn(Key) -> u16) {
        let Some(record) = self.record(key).copied() else {
            return;
        };
        let mut layer = self.binding_layer(&record);
        let mut index = record.advanced(layer);
        let state = index.and_then(|index| self.advanced.state(index).copied());
        self.decide(key, travel, index);
        if self.press_state(key) == (true, true) {
            self.resolve_pending(key, index, travels);
            let resolved = self.binding_layer(&record);
            if resolved != layer && record.advanced(resolved) != index {
                if let Some(slot) = self.record_mut(key) {
                    slot.actuation = record.actuation;
                    slot.active = record.active;
                }
                if let (Some(index), Some(state)) = (index, state)
                    && let Some(slot) = self.advanced.state_mut(index)
                {
                    *slot = state;
                }
                layer = resolved;
                index = record.advanced(resolved);
                self.decide(key, travel, index);
            }
        }
        if self
            .record(key)
            .is_some_and(|record| record.actuation.is_changed())
        {
            layer = self.handle_edge(key, index, layer);
        }
        if !self.update_common(key, travel, index) {
            self.emit(key, travel, layer);
        }
    }

    fn decide(&mut self, key: Key, travel: u16, index: Option<u8>) {
        if let Some(index) = index
            && self.key_reports
        {
            if usize::from(index) < self.advanced.entries().len() {
                self.advanced_input(key, travel, index);
            }
        } else {
            self.update_press_state(key, travel);
        }
    }

    fn update_press_state(&mut self, key: Key, travel: u16) {
        let tuning = self.tuning;
        let Some(record) = self.record_mut(key) else {
            return;
        };
        let threshold = threshold(record, &tuning);
        let trigger = trigger(record, &tuning);
        let zone = record.actuation.update(travel, threshold, trigger);
        let event = match zone {
            Some(Zone::Entered) => KeyEvent::Actuated,
            Some(Zone::Left) => KeyEvent::Reset,
            None => return,
        };
        self.output.notify(Notice::Key { key, event });
    }

    fn press_state(&self, key: Key) -> (bool, bool) {
        self.record(key).map_or((false, false), |record| {
            (record.actuation.is_pressed(), record.actuation.is_changed())
        })
    }

    fn advanced_input(&mut self, key: Key, travel: u16, index: u8) {
        let (Some(entry), Some(mut state)) = (
            self.advanced.entry(index).copied(),
            self.advanced.state(index).copied(),
        ) else {
            return;
        };
        let active = match (&entry.behaviour, &mut state) {
            (
                Behaviour::DynamicKeystroke {
                    strokes,
                    bottom_out,
                },
                State::DynamicKeystroke(keystroke),
            ) => {
                let thresholds = self.record(key).map(|record| {
                    Thresholds::new(
                        threshold(record, &self.tuning),
                        *bottom_out,
                        record.travel_limit,
                    )
                });
                let Some(thresholds) = thresholds else {
                    return;
                };
                let (edge, active) = keystroke.input(strokes, travel, thresholds);
                if let Some(pressed) = edge
                    && let Some(record) = self.record_mut(key)
                {
                    record.actuation.set(pressed);
                }
                active
            }
            (Behaviour::ModTap(mod_tap), State::ModTap(decision)) => {
                self.update_press_state(key, travel);
                let (pressed, changed) = self.press_state(key);
                let (slot, active) = decision.input(mod_tap, pressed, changed);
                match slot {
                    SlotChange::Keep => {}
                    SlotChange::Release(slot) => self.pending.release(slot),
                    SlotChange::Take => decision.set_slot(self.pending.take(key)),
                }
                active
            }
            (Behaviour::Toggle(action), State::Toggle(toggle)) => {
                self.update_press_state(key, travel);
                let (pressed, changed) = self.press_state(key);
                match toggle.input(pressed, changed) {
                    Some(Latch::On) => self.output.notify(Notice::Toggle {
                        key,
                        latched: true,
                        layer_key: matches!(action, Action::Layer(layer) if *layer != Layer::Base),
                    }),
                    Some(Latch::Off) => self.output.notify(Notice::Toggle {
                        key,
                        latched: false,
                        layer_key: false,
                    }),
                    None => {}
                }
                toggle.is_latched()
            }
            (
                Behaviour::DepthPriority { other, .. } | Behaviour::Socd { other, .. },
                State::Pair(pair),
            ) => {
                self.update_press_state(key, travel);
                let (pressed, _) = self.press_state(key);
                pair.capture(key == *other, pressed, travel)
            }
            _ => false,
        };
        if let Some(slot) = self.advanced.state_mut(index) {
            *slot = state;
        }
        if let Some(record) = self.record_mut(key) {
            record.set_active(entry.layer, active);
        }
    }

    fn handle_edge(&mut self, key: Key, index: Option<u8>, layer: u8) -> u8 {
        let mut layer = layer;
        let (pressed, _) = self.press_state(key);
        if pressed {
            let active = self.layers.active();
            if let Some(record) = self.record_mut(key) {
                record.latched_layer = active;
            }
            self.output.notify(Notice::Key {
                key,
                event: KeyEvent::Pressed,
            });
            if let Some(record) = self.record(key).copied() {
                layer = self.binding_layer(&record);
            }
        } else {
            let locked = self.layers.locked();
            if let Some(record) = self.record_mut(key) {
                record.latched_layer = locked;
            }
            self.output.notify(Notice::Key {
                key,
                event: KeyEvent::Released,
            });
            if let Some(record) = self.record(key).copied() {
                let rapid = matches!(trigger(&record, &self.tuning), Trigger::Rapid(_));
                let needs_reset = index.is_some()
                    && self.current_entry(&record).is_none_or(|(_, entry)| {
                        matches!(entry.behaviour, Behaviour::DynamicKeystroke { .. })
                    });
                if !rapid || needs_reset {
                    self.output.notify(Notice::Key {
                        key,
                        event: KeyEvent::Reset,
                    });
                }
            }
        }
        if let Some(record) = self.record_mut(key) {
            record.actuation.acknowledge();
        }
        layer
    }

    fn resolve_pending(
        &mut self,
        presser: Key,
        pressed_with: Option<u8>,
        travels: &impl Fn(Key) -> u16,
    ) {
        let mut deferred = false;
        let mut slot = 0;
        while self.pending.top().is_some_and(|top| slot <= top) {
            if self.pending.key(slot).is_some()
                && self.resolve_slot(presser, pressed_with, slot, travels)
            {
                deferred = true;
            }
            slot += 1;
        }
        if deferred && let Some(record) = self.record_mut(presser) {
            record.defer_output = true;
        }
    }

    fn resolve_slot(
        &mut self,
        presser: Key,
        pressed_with: Option<u8>,
        slot: u8,
        travels: &impl Fn(Key) -> u16,
    ) -> bool {
        let Some(waiting) = self.pending.key(slot) else {
            return false;
        };
        if waiting == presser {
            return false;
        }
        let (Some(waiting_record), Some(_)) =
            (self.record(waiting).copied(), self.record(presser).copied())
        else {
            return false;
        };
        let Some((index, entry)) = self
            .current_entry(&waiting_record)
            .map(|(index, entry)| (index, *entry))
        else {
            return false;
        };
        let Behaviour::ModTap(mod_tap) = entry.behaviour else {
            return false;
        };
        if pressed_with
            .and_then(|index| self.advanced.entry(index))
            .is_some_and(|entry| matches!(entry.behaviour, Behaviour::ModTap(_)))
        {
            return false;
        }
        let Some(State::ModTap(mut state)) = self.advanced.state(index).copied() else {
            return false;
        };
        let decided = if waiting_record.latched_layer == entry.layer {
            state.interrupt(&mod_tap)
        } else {
            None
        };
        state.set_slot(None);
        if let Some(slot) = self.advanced.state_mut(index) {
            *slot = State::ModTap(state);
        }
        if let Some(action) = decided {
            self.apply_action(action, waiting, entry.layer, travels(waiting), true, true);
        }
        self.pending.release(slot);
        decided.is_some()
    }

    fn update_common(&mut self, key: Key, travel: u16, index: Option<u8>) -> bool {
        let Some(record) = self.record_mut(key) else {
            return false;
        };
        if travel == 0 {
            record.ignore_until_release = false;
        }
        if !record.defer_output {
            return false;
        }
        let pair_owns = index.is_some()
            && self.record(key).copied().is_some_and(|record| {
                self.current_entry(&record)
                    .is_some_and(|(_, entry)| entry.behaviour.is_pair())
            });
        if !pair_owns && let Some(record) = self.record_mut(key) {
            record.defer_output = false;
        }
        true
    }

    fn emit(&mut self, key: Key, travel: u16, layer: u8) {
        let Some(record) = self.record(key).copied() else {
            return;
        };
        if let Some(index) = record.advanced(self.binding_layer(&record)) {
            self.run_advanced(key, travel, Some(index));
            self.apply_gamepad_binding(key, travel, layer);
            return;
        }
        if record.any_active() {
            self.run_advanced(key, travel, None);
        }
        let Some(record) = self.record(key).copied() else {
            return;
        };
        let action = record.action(layer);
        let pressed = record.actuation.is_pressed() && self.allows(&record, action, layer);
        self.apply_action(action, key, layer, travel, false, pressed);
        self.apply_gamepad_binding(key, travel, layer);
    }

    fn allows(&self, record: &Record, action: Action, layer: u8) -> bool {
        let enabled = !record.disabled || action == Action::Function(Function::ToggleKeyDisable);
        let not_gamepad_only = !self.gamepad_exclusive
            || record.gamepad.is_none()
            || self.mode == Mode::NoGamepad
            || layer != 0;
        enabled && not_gamepad_only && !record.ignore_until_release
    }

    fn apply_gamepad_binding(&mut self, key: Key, travel: u16, layer: u8) {
        if self.mode == Mode::NoGamepad || layer != 0 {
            return;
        }
        let Some(record) = self.record(key).copied() else {
            return;
        };
        if let Some(control) = record.gamepad {
            let mode = self.mode;
            self.output.gamepad_input_mut().apply(
                mode,
                control,
                travel,
                record.actuation.is_pressed(),
            );
        }
    }

    fn run_advanced(&mut self, key: Key, travel: u16, current: Option<u8>) {
        for layer in (0u8..).take(LAYERS) {
            let Some(record) = self.record(key).copied() else {
                return;
            };
            let Some(index) = record.advanced(layer) else {
                continue;
            };
            let active_binding = record.active.get(usize::from(layer)).copied() == Some(true);
            if !active_binding && Some(index) != current {
                continue;
            }
            let (Some(entry), Some(mut state)) = (
                self.advanced.entry(index).copied(),
                self.advanced.state(index).copied(),
            ) else {
                if let Some(record) = self.record_mut(key) {
                    record.set_active(layer, false);
                }
                continue;
            };
            let elapsed_ms = self.elapsed_ms;
            let tap_ms = self.tuning.tap_ms;
            let (applications, active) = match (&entry.behaviour, &mut state) {
                (
                    Behaviour::DynamicKeystroke { strokes, .. },
                    State::DynamicKeystroke(keystroke),
                ) => keystroke.run(strokes, travel, elapsed_ms, tap_ms),
                (Behaviour::ModTap(mod_tap), State::ModTap(decision)) => {
                    decision.run(mod_tap, travel, elapsed_ms, tap_ms)
                }
                (Behaviour::Toggle(action), State::Toggle(toggle)) => {
                    (toggle.run(*action, travel, elapsed_ms), toggle.is_latched())
                }
                (
                    Behaviour::DepthPriority { other, .. } | Behaviour::Socd { other, .. },
                    State::Pair(pair),
                ) => {
                    let active = pair.record(key == *other, record.actuation.is_pressed(), travel);
                    if let Some(slot) = self.advanced.state_mut(index) {
                        *slot = state;
                    }
                    if let Some(record) = self.record_mut(key) {
                        record.set_active(layer, active);
                    }
                    continue;
                }
                _ => (Vec::new(), false),
            };
            if let Some(slot) = self.advanced.state_mut(index) {
                *slot = state;
            }
            for application in applications {
                let allowed = self
                    .record(key)
                    .is_some_and(|record| self.allows(record, application.action, entry.layer));
                self.apply_action(
                    application.action,
                    key,
                    entry.layer,
                    application.travel,
                    true,
                    application.pressed && allowed,
                );
            }
            let toggle = matches!(entry.behaviour, Behaviour::Toggle(_));
            let pressed = (!toggle && active)
                || self
                    .record(key)
                    .is_some_and(|record| record.actuation.is_pressed());
            if self.key_reports {
                self.output
                    .log_advanced(entry.behaviour.kind(), key, travel, pressed);
            }
            if let Some(record) = self.record_mut(key) {
                record.set_active(layer, active);
            }
        }
    }

    fn resolve_pairs(&mut self) {
        for index in 0..u8::try_from(self.advanced.entries().len()).unwrap_or(u8::MAX) {
            let Some(entry) = self.advanced.entry(index).copied() else {
                break;
            };
            let Some(other) = entry.behaviour.other() else {
                continue;
            };
            if other == entry.key {
                continue;
            }
            let Some(State::Pair(mut state)) = self.advanced.state(index).copied() else {
                continue;
            };
            let (first_on, second_on) = match entry.behaviour {
                Behaviour::DepthPriority {
                    both_bottomed_out, ..
                } => state.resolve_depth(both_bottomed_out),
                Behaviour::Socd {
                    resolution,
                    both_when_bottomed_out,
                    ..
                } => state.resolve_socd(resolution, both_when_bottomed_out),
                _ => continue,
            };
            if let Some(slot) = self.advanced.state_mut(index) {
                *slot = State::Pair(state);
            }
            let [first, second] = state.sides();
            let kind = entry.behaviour.kind();
            if first.travel != 0 {
                self.emit_pair_key(kind, entry.key, entry.layer, first.travel, first_on);
            }
            if second.travel != 0 {
                self.emit_pair_key(kind, other, entry.layer, second.travel, second_on);
            }
        }
    }

    fn emit_pair_key(&mut self, kind: u8, key: Key, layer: u8, travel: u16, on: bool) {
        let Some(record) = self.record(key).copied() else {
            return;
        };
        if self.key_reports {
            self.output
                .log_advanced(kind, key, travel, record.actuation.is_pressed());
        }
        let action = record.action(layer);
        let pressed = on && !record.defer_output && self.allows(&record, action, layer);
        self.apply_action(action, key, layer, travel, false, pressed);
        if let Some(record) = self.record_mut(key) {
            record.defer_output = false;
        }
    }

    fn build_gamepad_report(&mut self) {
        let settings = self.gamepad_settings.as_ref();
        let report = match self.mode {
            Mode::Xinput => GamepadReport::Xinput(if self.gamepad_detection {
                Gamepad::xinput_detection(self.xinput_player)
            } else {
                self.gamepad
                    .xinput_report(self.output.gamepad_input(), settings)
            }),
            Mode::Gamepad => GamepadReport::Hid(if self.gamepad_detection {
                Gamepad::hid_detection()
            } else {
                let mut input = *self.output.gamepad_input();
                let report = self.gamepad.hid_report(&mut input, settings);
                *self.output.gamepad_input_mut() = input;
                report
            }),
            Mode::NoGamepad => return,
        };
        self.output.set_gamepad(report);
    }

    pub fn apply_action(
        &mut self,
        action: Action,
        key: Key,
        layer: u8,
        travel: u16,
        force_keyboard: bool,
        pressed: bool,
    ) {
        if !self.key_reports {
            return;
        }
        self.output.log(action, key, travel, pressed);
        if !pressed && !follows_travel_action(action) {
            return;
        }
        match action {
            Action::None => {}
            Action::Keyboard(usage) => {
                if self.mode != Mode::NoGamepad
                    && !self.tuning.keyboard_in_gamepad_mode
                    && !force_keyboard
                {
                    return;
                }
                self.output.press_keyboard(usage);
            }
            Action::Consumer(usage) => self.output.press_consumer(usage),
            Action::System(control) => self.output.press_system(control),
            Action::Mouse(button) => self.output.press_mouse(button),
            Action::Layer(_) | Action::Function(_) => {
                if let Some(special) = Special::from_action(action) {
                    self.request(special, key, layer);
                }
            }
            Action::Gamepad(control) => {
                let mode = self.mode;
                self.output
                    .gamepad_input_mut()
                    .apply(mode, control, travel, pressed);
            }
        }
    }

    fn request(&mut self, special: Special, key: Key, layer: u8) {
        let code = special.code();
        let request = Request {
            key,
            layer: layer & 3,
        };
        self.requests.set(code, Some(request));
        if self.held.get(code).is_some() {
            return;
        }
        self.held.set(code, Some(request));
        self.call(Call {
            special,
            key,
            layer: request.layer,
            pressed: true,
        });
    }

    fn dispatch_specials(&mut self) {
        for code in (0u8..).take(SPECIALS) {
            let request = self.requests.get(code);
            if self.held.get(code).is_some() == request.is_some() {
                continue;
            }
            self.held.set(code, request);
            let Some(special) = Special::from_code(code) else {
                continue;
            };
            let (key, layer) =
                request.map_or((Key::new(0, 0), 0), |request| (request.key, request.layer));
            self.call(Call {
                special,
                key,
                layer,
                pressed: request.is_some(),
            });
        }
    }

    fn call(&mut self, call: Call) {
        let mut effects = Effects {
            layers: &mut self.layers,
            profiles: &mut self.profiles,
            records: &mut self.records,
            output: &mut self.output,
        };
        self.functions.call(call, &mut effects);
    }
}

fn threshold(record: &Record, tuning: &Tuning) -> u16 {
    record
        .actuation_point
        .unwrap_or(tuning.actuation_point)
        .max(record.minimum)
}

fn trigger(record: &Record, tuning: &Tuning) -> Trigger {
    let rapid: Option<RapidTrigger> = match record.rapid_trigger {
        RapidTriggerOverride::Profile => {
            tuning.rapid_trigger_enabled.then_some(tuning.rapid_trigger)
        }
        RapidTriggerOverride::Off => None,
        RapidTriggerOverride::On => Some(tuning.rapid_trigger),
        RapidTriggerOverride::With(rapid_trigger) => Some(rapid_trigger),
    };
    rapid.map_or(Trigger::Fixed, Trigger::Rapid)
}
