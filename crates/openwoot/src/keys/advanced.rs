mod dynamic_keystroke;
mod mod_tap;
mod pair;
mod pending;
mod toggle;

pub(crate) use dynamic_keystroke::Thresholds;
pub use dynamic_keystroke::{DynamicKeystrokeState, Phase};
use heapless::Vec;
pub(crate) use mod_tap::SlotChange;
pub use mod_tap::{Decision, ModTap, ModTapState};
use openwoot_protocol::{
    advanced::{self, MAX_ENTRIES, STROKES, Stroke},
    keys::Action,
};
pub use pair::{Last, PairState, Side};
pub(crate) use pending::Pending;
pub(crate) use toggle::Latch;
pub use toggle::ToggleState;

use super::{Key, travel_of};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: Key,
    pub layer: u8,
    pub behaviour: Behaviour,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behaviour {
    DynamicKeystroke {
        strokes: [Stroke; STROKES],
        bottom_out: Option<u16>,
    },
    ModTap(ModTap),
    Toggle(Action),
    DepthPriority {
        other: Key,
        both_bottomed_out: advanced::Resolution,
    },
    Socd {
        other: Key,
        resolution: advanced::Resolution,
        both_when_bottomed_out: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    DynamicKeystroke(DynamicKeystrokeState),
    ModTap(ModTapState),
    Toggle(ToggleState),
    Pair(PairState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Application {
    pub(super) action: Action,
    pub(super) travel: u16,
    pub(super) pressed: bool,
}

pub(super) type Applications = Vec<Application, STROKES>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Advanced {
    entries: Vec<Entry, MAX_ENTRIES>,
    states: Vec<State, MAX_ENTRIES>,
}

impl Behaviour {
    #[must_use]
    pub const fn kind(&self) -> u8 {
        match self {
            Self::DynamicKeystroke { .. } => 1,
            Self::ModTap(_) => 2,
            Self::Toggle(_) => 3,
            Self::DepthPriority { .. } => 4,
            Self::Socd { .. } => 5,
        }
    }

    #[must_use]
    pub const fn is_pair(&self) -> bool {
        matches!(self, Self::DepthPriority { .. } | Self::Socd { .. })
    }

    #[must_use]
    pub const fn other(&self) -> Option<Key> {
        match self {
            Self::DepthPriority { other, .. } | Self::Socd { other, .. } => Some(*other),
            _ => None,
        }
    }

    #[must_use]
    pub fn actions(&self) -> Vec<Action, STROKES> {
        match self {
            Self::DynamicKeystroke { strokes, .. } => {
                strokes.iter().map(|stroke| stroke.action).collect()
            }
            Self::ModTap(mod_tap) => [mod_tap.hold, mod_tap.tap].into_iter().collect(),
            Self::Toggle(action) => core::iter::once(*action).collect(),
            Self::DepthPriority { .. } | Self::Socd { .. } => Vec::new(),
        }
    }

    fn initial_state(&self) -> State {
        match self {
            Self::DynamicKeystroke { .. } => {
                State::DynamicKeystroke(DynamicKeystrokeState::default())
            }
            Self::ModTap(_) => State::ModTap(ModTapState::default()),
            Self::Toggle(_) => State::Toggle(ToggleState::default()),
            Self::DepthPriority { .. } | Self::Socd { .. } => State::Pair(PairState::default()),
        }
    }
}

impl Entry {
    #[must_use]
    pub fn from_settings(
        entry: &advanced::Key,
        columns: usize,
        minimum: impl Fn(Key) -> u16,
    ) -> Option<Self> {
        let key = Key::from_index(entry.key.get(), columns)?;
        let behaviour = match entry.behaviour {
            advanced::Behaviour::DynamicKeystroke(keystroke) => Behaviour::DynamicKeystroke {
                strokes: keystroke.strokes,
                bottom_out: keystroke
                    .bottom_out
                    .map(travel_of)
                    .filter(|&bottom_out| bottom_out >= minimum(key)),
            },
            advanced::Behaviour::ModTap(mod_tap) => Behaviour::ModTap(ModTap {
                tap: mod_tap.tap,
                hold: mod_tap.hold,
                hold_ms: mod_tap.hold_time_ms,
                interrupt: mod_tap.interrupt,
            }),
            advanced::Behaviour::Toggle(action) => Behaviour::Toggle(action),
            advanced::Behaviour::DepthPriority(pair) => Behaviour::DepthPriority {
                other: Key::from_index(pair.other.get(), columns)?,
                both_bottomed_out: pair.both_bottomed_out,
            },
            advanced::Behaviour::Socd(socd) => Behaviour::Socd {
                other: Key::from_index(socd.other.get(), columns)?,
                resolution: socd.resolution,
                both_when_bottomed_out: socd.both_when_bottomed_out,
            },
        };
        Some(Self {
            key,
            layer: entry.layer.number(),
            behaviour,
        })
    }
}

impl Advanced {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            states: Vec::new(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    #[must_use]
    pub fn entry(&self, index: u8) -> Option<&Entry> {
        self.entries.get(usize::from(index))
    }

    #[must_use]
    pub fn state(&self, index: u8) -> Option<&State> {
        self.states.get(usize::from(index))
    }

    pub(super) fn state_mut(&mut self, index: u8) -> Option<&mut State> {
        self.states.get_mut(usize::from(index))
    }

    pub(super) fn replace(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.entries.clear();
        self.states.clear();
        for entry in entries {
            if self.entries.push(entry).is_err() {
                break;
            }
            let _ = self.states.push(entry.behaviour.initial_state());
        }
    }

    pub(super) fn latched_toggles(&self) -> impl Iterator<Item = Key> + '_ {
        self.entries
            .iter()
            .zip(&self.states)
            .filter_map(|(entry, state)| match state {
                State::Toggle(toggle) if toggle.is_latched() => Some(entry.key),
                _ => None,
            })
    }
}
