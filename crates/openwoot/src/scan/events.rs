use super::Position;
use crate::keys::Key;

pub const EVENT_CAPACITY: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub position: Position,
    pub key: Key,
    pub travel: u16,
}

#[derive(Clone, Debug)]
pub struct Events {
    events: [Event; EVENT_CAPACITY],
    len: usize,
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

impl Events {
    #[must_use]
    pub const fn new() -> Self {
        const EMPTY: Event = Event {
            position: Position::new(0, 0),
            key: Key::new(0, 0),
            travel: 0,
        };
        Self {
            events: [EMPTY; EVENT_CAPACITY],
            len: 0,
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[Event] {
        self.events.get(..self.len).unwrap_or_default()
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Event> {
        self.as_slice().iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) const fn clear(&mut self) {
        self.len = 0;
    }

    pub(super) fn push(&mut self, event: Event) {
        if let Some(slot) = self.events.get_mut(self.len) {
            *slot = event;
            self.len += 1;
        }
    }
}

impl<'events> IntoIterator for &'events Events {
    type IntoIter = core::slice::Iter<'events, Event>;
    type Item = &'events Event;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
