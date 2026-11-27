use embassy_sync::blocking_mutex::raw::RawMutex;
use heapless::Vec;
use openwoot_protocol::{
    keys::Action,
    macros::{MAX_MACROS, Mode, Step, Text},
};

use super::{Header, Loaded, Shared, Stroke};

const HELD: usize = 16;
const STEPS_PER_PASS: u8 = 8;
const AXIS_LIMIT: i16 = 127;
const KEYBOARD: u8 = 1;
const CONTROLS: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drained {
    pub keyboard: bool,
    pub controls: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    pub x: i8,
    pub y: i8,
    pub wheel: i8,
    pub pan: i8,
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    holding: u32,
    held: u32,
    state: State,
    hold: Hold,
    flow: Flow,
}

#[derive(Clone, Debug, Default)]
struct Hold {
    pressed: Vec<Action, HELD>,
    tapped: Option<Action>,
    typed: Typed,
    motion: Motion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Typed {
    shift: bool,
    usage: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Flow {
    last: u8,
    ready: bool,
    changed: bool,
    moved: bool,
}

#[derive(Clone, Debug, Default)]
enum State {
    #[default]
    Idle,
    Loading(u8),
    Playing(Cursor),
}

#[derive(Clone, Debug)]
struct Cursor {
    index: u8,
    header: Header,
    begun: u16,
    offset: usize,
    current: Current,
}

#[derive(Clone, Debug)]
enum Current {
    Next,
    Press(Action),
    Release(Action),
    TapDown(Action),
    TapUp(Action),
    Delay(u32),
    Text {
        text: Text,
        position: usize,
        phase: Phase,
    },
    Move {
        x: i16,
        y: i16,
    },
    Scroll {
        vertical: i8,
        horizontal: i8,
    },
    Ending,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Begin,
    Shift(Stroke),
    Down(Stroke),
    Up,
}

enum Advance {
    Continue,
    Wait,
    Finished,
}

impl Player {
    pub fn hold(&mut self, index: u8) {
        if index < MAX_MACROS {
            self.holding |= bit(index);
        }
    }

    pub fn stop(&mut self) {
        self.state = State::Idle;
        self.hold.release();
    }

    #[inline(never)]
    pub fn run<M: RawMutex>(&mut self, shared: &Shared<M>, elapsed_ms: u32, drained: Drained) {
        self.flow
            .begin(drained, self.hold.motion != Motion::default());
        self.hold.motion = Motion::default();
        let previous = self.held;
        self.held = core::mem::take(&mut self.holding);
        let pressed = self.held & !previous;
        if pressed != 0 {
            self.trigger(lowest(pressed), shared);
        }
        if let State::Loading(index) = self.state {
            match shared.loaded(index) {
                Loaded::Pending => return,
                Loaded::Missing => {
                    self.state = State::Idle;
                    return;
                }
                Loaded::Ready(header) => self.state = State::Playing(Cursor::new(index, header)),
            }
        }
        let ended = match &mut self.state {
            State::Playing(cursor) => cursor.play(
                shared,
                &mut self.hold,
                &mut self.flow,
                self.held,
                elapsed_ms,
            ),
            State::Idle | State::Loading(_) => false,
        };
        if ended {
            self.state = State::Idle;
        }
    }

    pub fn actions(&self) -> impl Iterator<Item = Action> + '_ {
        self.hold.actions()
    }

    #[must_use]
    pub const fn motion(&self) -> Motion {
        self.hold.motion
    }

    fn trigger<M: RawMutex>(&mut self, index: u8, shared: &Shared<M>) {
        let toggled_off = matches!(
            &self.state,
            State::Playing(cursor) if cursor.index == index && cursor.header.mode == Mode::Toggle
        );
        self.stop();
        if !toggled_off {
            self.state = State::Loading(index);
            shared.want(index);
        }
    }
}

impl Hold {
    fn actions(&self) -> impl Iterator<Item = Action> + '_ {
        let typed = self.typed;
        self.pressed
            .iter()
            .copied()
            .chain(self.tapped)
            .chain(typed.shift.then_some(Action::Keyboard(Stroke::SHIFT)))
            .chain(typed.usage.map(Action::Keyboard))
    }

    fn interfaces(&self) -> u8 {
        self.actions()
            .fold(0, |interfaces, action| interfaces | interface(action))
    }

    fn release(&mut self) {
        self.pressed.clear();
        self.tapped = None;
        self.typed = Typed::default();
    }
}

impl Flow {
    const fn begin(&mut self, drained: Drained, moved: bool) {
        let keyboard = self.last & KEYBOARD == 0 || drained.keyboard;
        let controls = self.last & CONTROLS == 0 || drained.controls;
        self.ready = keyboard && controls;
        self.changed = false;
        self.moved = moved;
    }

    const fn may_change(self) -> bool {
        self.ready && !self.changed
    }

    const fn may_move(self) -> bool {
        self.may_change() && !self.moved
    }

    const fn record(&mut self, interfaces: u8) {
        self.changed = true;
        self.last = interfaces;
    }
}

impl Cursor {
    const fn new(index: u8, header: Header) -> Self {
        Self {
            index,
            header,
            begun: 0,
            offset: 0,
            current: Current::Next,
        }
    }

    fn play<M: RawMutex>(
        &mut self,
        shared: &Shared<M>,
        hold: &mut Hold,
        flow: &mut Flow,
        triggers: u32,
        elapsed_ms: u32,
    ) -> bool {
        if let Current::Delay(left) = &mut self.current {
            *left = left.saturating_sub(elapsed_ms);
        }
        for _ in 0..STEPS_PER_PASS {
            match self.advance(shared, hold, flow) {
                Advance::Continue => {}
                Advance::Wait => return false,
                Advance::Finished => {
                    let repeats = self.header.steps > 0
                        && match self.header.mode {
                            Mode::Once => false,
                            Mode::WhileHeld => triggers & bit(self.index) != 0,
                            Mode::Toggle => true,
                        };
                    if !repeats {
                        return true;
                    }
                    self.begun = 0;
                    self.offset = 0;
                    self.current = Current::Next;
                    return false;
                }
            }
        }
        false
    }

    fn advance<M: RawMutex>(
        &mut self,
        shared: &Shared<M>,
        hold: &mut Hold,
        flow: &mut Flow,
    ) -> Advance {
        match &mut self.current {
            Current::Next => {
                let step = if self.begun < self.header.steps {
                    shared.step(self.index, self.offset)
                } else {
                    None
                };
                self.current = match step {
                    Some((step, offset)) => {
                        self.begun += 1;
                        self.offset = offset;
                        Current::from_step(step, hold)
                    }
                    None => Current::Ending,
                };
            }
            Current::Delay(left) => {
                if *left > 0 {
                    return Advance::Wait;
                }
                self.current = Current::Next;
            }
            Current::Text {
                text,
                position,
                phase,
            } => {
                if *position < text.len() {
                    return type_text(self.header, text, position, phase, hold, flow);
                }
                self.current = Current::Next;
            }
            Current::Ending => {
                let released = hold.interfaces();
                if released != 0 {
                    if !flow.may_change() {
                        return Advance::Wait;
                    }
                    hold.release();
                    flow.record(released);
                }
                return Advance::Finished;
            }
            current => return current.act(hold, flow),
        }
        Advance::Continue
    }
}

impl Current {
    fn act(&mut self, hold: &mut Hold, flow: &mut Flow) -> Advance {
        if let Self::Move { x: 0, y: 0 } = self {
            *self = Self::Next;
            return Advance::Continue;
        }
        let allowed = match self {
            Self::Move { .. } | Self::Scroll { .. } => flow.may_move(),
            _ => flow.may_change(),
        };
        if !allowed {
            return Advance::Wait;
        }
        *self = match self {
            Self::Press(action) => {
                let action = *action;
                if hold.pressed.push(action).is_ok() {
                    flow.record(interface(action));
                }
                Self::Next
            }
            Self::Release(action) => {
                let action = *action;
                hold.pressed.retain(|pressed| *pressed != action);
                flow.record(interface(action));
                Self::Next
            }
            Self::TapDown(action) => {
                let action = *action;
                hold.tapped = Some(action);
                flow.record(interface(action));
                Self::TapUp(action)
            }
            Self::TapUp(action) => {
                hold.tapped = None;
                flow.record(interface(*action));
                Self::Next
            }
            Self::Move { x, y } => {
                let (step_x, step_y) = (limited(*x), limited(*y));
                hold.motion.x = i8::try_from(step_x).unwrap_or_default();
                hold.motion.y = i8::try_from(step_y).unwrap_or_default();
                *x -= step_x;
                *y -= step_y;
                flow.record(CONTROLS);
                return Advance::Continue;
            }
            Self::Scroll {
                vertical,
                horizontal,
            } => {
                hold.motion.wheel = (*vertical).max(-127);
                hold.motion.pan = (*horizontal).max(-127);
                flow.record(CONTROLS);
                Self::Next
            }
            Self::Next | Self::Delay(_) | Self::Text { .. } | Self::Ending => {
                return Advance::Continue;
            }
        };
        Advance::Continue
    }

    fn from_step(step: Step, hold: &Hold) -> Self {
        match step {
            Step::Press(action) if is_playable(action) && !hold.pressed.contains(&action) => {
                Self::Press(action)
            }
            Step::Release(action) if hold.pressed.contains(&action) => Self::Release(action),
            Step::Tap(action) if is_playable(action) => Self::TapDown(action),
            Step::Delay(ms) => Self::Delay(u32::from(ms)),
            Step::Text(text) => Self::Text {
                text,
                position: 0,
                phase: Phase::Begin,
            },
            Step::MouseMove { x, y } => Self::Move { x, y },
            Step::Scroll {
                vertical,
                horizontal,
            } if vertical != 0 || horizontal != 0 => Self::Scroll {
                vertical,
                horizontal,
            },
            Step::Press(_) | Step::Release(_) | Step::Tap(_) | Step::Scroll { .. } => Self::Next,
        }
    }
}

fn type_text(
    header: Header,
    text: &Text,
    position: &mut usize,
    phase: &mut Phase,
    hold: &mut Hold,
    flow: &mut Flow,
) -> Advance {
    match *phase {
        Phase::Begin => {
            let Some(&character) = text.as_bytes().get(*position) else {
                return Advance::Continue;
            };
            *phase = match Stroke::of(header.layout, character) {
                Some(stroke) if stroke.shift => Phase::Shift(stroke),
                Some(stroke) => Phase::Down(stroke),
                None => {
                    *position += 1;
                    Phase::Begin
                }
            };
        }
        Phase::Shift(stroke) => {
            if !flow.may_change() {
                return Advance::Wait;
            }
            hold.typed = Typed {
                shift: true,
                usage: None,
            };
            flow.record(KEYBOARD);
            *phase = Phase::Down(stroke);
        }
        Phase::Down(stroke) => {
            if !flow.may_change() {
                return Advance::Wait;
            }
            hold.typed = Typed {
                shift: stroke.shift,
                usage: Some(stroke.usage),
            };
            flow.record(KEYBOARD);
            *phase = Phase::Up;
        }
        Phase::Up => {
            if !flow.may_change() {
                return Advance::Wait;
            }
            hold.typed = Typed::default();
            flow.record(KEYBOARD);
            *position += 1;
            *phase = Phase::Begin;
        }
    }
    Advance::Continue
}

const fn is_playable(action: Action) -> bool {
    matches!(
        action,
        Action::Keyboard(_) | Action::Consumer(_) | Action::System(_) | Action::Mouse(_)
    )
}

const fn interface(action: Action) -> u8 {
    match action {
        Action::Keyboard(_) => KEYBOARD,
        Action::Consumer(_) | Action::System(_) | Action::Mouse(_) => CONTROLS,
        Action::None
        | Action::Gamepad(_)
        | Action::Layer(_)
        | Action::Function(_)
        | Action::Macro(_) => 0,
    }
}

const fn limited(remaining: i16) -> i16 {
    if remaining > AXIS_LIMIT {
        AXIS_LIMIT
    } else if remaining < -AXIS_LIMIT {
        -AXIS_LIMIT
    } else {
        remaining
    }
}

fn bit(index: u8) -> u32 {
    1_u32.checked_shl(u32::from(index)).unwrap_or(0)
}

fn lowest(mask: u32) -> u8 {
    u8::try_from(mask.trailing_zeros()).unwrap_or(u8::MAX)
}
