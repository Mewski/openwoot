use gpui_kit::SharedString;
use openwoot_protocol::{
    keys::Action,
    macros::{Change, MAX_MACROS, Macro, Summaries, Summary},
};

use crate::usage;

const SHORT_NAME: usize = 7;
const ELLIPSIS: char = '…';

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Library {
    slots: Vec<Option<Summary>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Write {
    Set(Box<Change>),
    Clear(u8),
}

impl Library {
    pub(crate) fn of(summaries: &Summaries) -> Self {
        Self {
            slots: summaries.iter().cloned().collect(),
        }
    }

    pub(crate) fn is_available(&self) -> bool {
        !self.slots.is_empty()
    }

    pub(crate) fn get(&self, index: u8) -> Option<&Summary> {
        self.slots.get(usize::from(index)).and_then(Option::as_ref)
    }

    pub(crate) fn name(&self, index: u8) -> Option<&str> {
        self.get(index).map(|summary| summary.name.as_str())
    }

    pub(crate) fn legend(&self, index: u8) -> SharedString {
        self.name(index).map_or_else(
            || SharedString::from(usage::legend(Action::Macro(index))),
            shortened,
        )
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u8, &Summary)> {
        (0..MAX_MACROS)
            .zip(&self.slots)
            .filter_map(|(index, slot)| slot.as_ref().map(|summary| (index, summary)))
    }

    pub(crate) fn len(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub(crate) fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn free(&self) -> Option<u8> {
        (0..MAX_MACROS)
            .zip(&self.slots)
            .find_map(|(index, slot)| slot.is_none().then_some(index))
    }

    pub(crate) fn set(&mut self, index: u8, summary: Option<Summary>) {
        if let Some(slot) = self.slots.get_mut(usize::from(index)) {
            *slot = summary;
        }
    }
}

impl Write {
    pub(crate) fn of(index: u8, value: Option<Macro>) -> Self {
        match value {
            Some(value) => Self::Set(Box::new(Change { index, value })),
            None => Self::Clear(index),
        }
    }
}

fn shortened(name: &str) -> SharedString {
    if name.chars().count() <= SHORT_NAME {
        return SharedString::from(name.to_owned());
    }
    let room = SHORT_NAME - 1;
    let mut short = String::new();
    for word in name.split_whitespace() {
        let gap = usize::from(!short.is_empty());
        if short.chars().count() + gap + word.chars().count() > room {
            break;
        }
        if gap > 0 {
            short.push(' ');
        }
        short.push_str(word);
    }
    if short.is_empty() {
        short = name.chars().take(room).collect();
    }
    short.push(ELLIPSIS);
    SharedString::from(short)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        heapless,
        macros::{Mode, Name, Step},
    };

    use super::*;

    fn named(name: &str) -> Macro {
        Macro {
            name: Name::try_from(name).unwrap(),
            mode: Mode::Once,
            steps: heapless::Vec::from_slice(&[Step::Tap(Action::Keyboard(0x04))]).unwrap(),
        }
    }

    fn library(names: &[Option<&str>]) -> Library {
        let mut summaries = Summaries::new();
        for slot in 0..MAX_MACROS {
            let summary = names
                .get(usize::from(slot))
                .copied()
                .flatten()
                .map(|name| Summary::from(&named(name)));
            summaries.push(summary).unwrap();
        }
        Library::of(&summaries)
    }

    #[test]
    fn a_new_macro_takes_the_first_free_slot() {
        let library = library(&[Some("Copy"), None, Some("Paste")]);
        assert_eq!(library.free(), Some(1));
        assert_eq!(library.len(), 2);
        assert_eq!(library.capacity(), usize::from(MAX_MACROS));
        let full = Library::of(
            &(0..MAX_MACROS)
                .map(|_| Some(Summary::from(&named("Full"))))
                .collect(),
        );
        assert_eq!(full.free(), None);
    }

    #[test]
    fn written_macros_update_their_summary_and_cleared_slots_empty_it() {
        let mut library = library(&[Some("Copy")]);
        library.set(4, Some(Summary::from(&named("Greeting"))));
        assert_eq!(library.name(4), Some("Greeting"));
        assert_eq!(library.get(4).map(|summary| summary.steps), Some(1));
        library.set(0, None);
        assert_eq!(library.name(0), None);
        assert_eq!(
            library.iter().map(|(index, _)| index).collect::<Vec<_>>(),
            [4]
        );
        library.set(MAX_MACROS, Some(Summary::from(&named("Outside"))));
        assert_eq!(library.len(), 1);
    }

    #[test]
    fn legends_shorten_long_names_and_name_empty_slots_by_number() {
        let library = library(&[Some("Copy"), Some("Greeting for the team"), None]);
        assert_eq!(library.legend(0), "Copy");
        assert_eq!(library.legend(1), "Greeti…");
        assert_eq!(library.legend(2), "M3");
        assert_eq!(shortened("Mail me"), "Mail me");
        assert_eq!(shortened("Open a new tab"), "Open a…");
        assert_eq!(shortened("Copy line"), "Copy…");
    }

    #[test]
    fn a_macro_is_written_whole_and_an_empty_slot_is_cleared() {
        let value = named("Copy");
        assert_eq!(
            Write::of(3, Some(value.clone())),
            Write::Set(Box::new(Change { index: 3, value }))
        );
        assert_eq!(Write::of(3, None), Write::Clear(3));
    }
}
