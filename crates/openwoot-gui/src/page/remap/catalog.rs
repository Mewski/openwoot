use openwoot_protocol::{
    keys::{Action, Function, Layer},
    profile::Slot,
};

use crate::usage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Category {
    Basic,
    Extended,
    International,
    OtherKeys,
    Functions,
    Layers,
    Profiles,
    Media,
    Applications,
    System,
    Mouse,
    Gamepad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) action: Action,
    pub(crate) category: Category,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    Exact,
    Prefix,
    Contains,
}

impl Category {
    pub(crate) const ALL: [Self; 12] = [
        Self::Basic,
        Self::Extended,
        Self::International,
        Self::OtherKeys,
        Self::Functions,
        Self::Layers,
        Self::Profiles,
        Self::Media,
        Self::Applications,
        Self::System,
        Self::Mouse,
        Self::Gamepad,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Basic => "Basic characters",
            Self::Extended => "Extended keys",
            Self::International => "International",
            Self::OtherKeys => "Other keys",
            Self::Functions => "Functions",
            Self::Layers => "Fn layers",
            Self::Profiles => "Profiles",
            Self::Media => "Media",
            Self::Applications => "Applications",
            Self::System => "System",
            Self::Mouse => "Mouse",
            Self::Gamepad => "Gamepad",
        }
    }
}

pub(crate) fn entries(profile_slots: u8) -> Vec<Entry> {
    let entry = |action, category| Entry { action, category };
    let keyboard = usage::KEYBOARD
        .iter()
        .map(|(code, _, _)| entry(Action::Keyboard(*code), keyboard_category(*code)));
    let functions = [
        Function::BrightnessUp,
        Function::BrightnessDown,
        Function::ToggleKeyDisable,
        Function::LayerLock,
    ]
    .into_iter()
    .map(|function| entry(Action::Function(function), Category::Functions));
    let layers = [Layer::One, Layer::Two, Layer::Three]
        .into_iter()
        .map(|layer| entry(Action::Layer(layer), Category::Layers));
    let profiles = (0..profile_slots)
        .map(|slot| Action::Function(Function::Profile(Slot::new(slot))))
        .chain([
            Action::Function(Function::NextProfile),
            Action::Function(Function::ToggleModeProfile),
        ])
        .map(|action| entry(action, Category::Profiles));
    let consumer = usage::CONSUMER
        .iter()
        .map(|(code, _, _)| entry(Action::Consumer(*code), consumer_category(*code)));
    let system = usage::SYSTEM
        .iter()
        .map(|(control, _, _)| entry(Action::System(*control), Category::System));
    let mouse = usage::MOUSE
        .iter()
        .map(|(button, _, _)| entry(Action::Mouse(*button), Category::Mouse));
    let gamepad = usage::GAMEPAD
        .iter()
        .map(|(control, _, _)| entry(Action::Gamepad(*control), Category::Gamepad));
    keyboard
        .chain(functions)
        .chain(layers)
        .chain(profiles)
        .chain(consumer)
        .chain(system)
        .chain(mouse)
        .chain(gamepad)
        .collect()
}

pub(crate) fn search(entries: &[Entry], query: &str) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return entries.to_vec();
    }
    let mut ranked: Vec<(Rank, usize, Entry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(position, entry)| {
            rank(entry.action, &query).map(|rank| (rank, position, *entry))
        })
        .collect();
    ranked.sort_by_key(|(rank, position, _)| (*rank, *position));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

pub(crate) fn refusal(action: Action, layer: Layer) -> Option<&'static str> {
    match action {
        Action::Layer(Layer::Base) => Some("The base layer can't be momentary"),
        Action::Layer(opened) if opened == layer => Some("Can't go on the layer it opens"),
        Action::Function(Function::LayerLock) if layer == Layer::Base => {
            Some("Only works on Fn layers")
        }
        _ => None,
    }
}

const fn keyboard_category(code: u8) -> Category {
    match code {
        0x04..=0x27 | 0x2D..=0x38 | 0x64 => Category::Basic,
        0x28..=0x2C | 0x39..=0x63 | 0x65 | 0x68..=0x73 | 0xE0..=0xE7 => Category::Extended,
        0x87..=0x98 => Category::International,
        _ => Category::OtherKeys,
    }
}

const fn consumer_category(code: u16) -> Category {
    match code {
        0x00B5..=0x00EA => Category::Media,
        0x006F | 0x0070 => Category::System,
        _ => Category::Applications,
    }
}

fn rank(action: Action, query: &str) -> Option<Rank> {
    let short = usage::legend(action).to_lowercase();
    let long = usage::name(action).to_lowercase();
    if short == query || long == query {
        Some(Rank::Exact)
    } else if short.starts_with(query)
        || long
            .split(|character: char| character.is_whitespace() || character == '/')
            .any(|word| word.starts_with(query))
    {
        Some(Rank::Prefix)
    } else if short.contains(query) || long.contains(query) {
        Some(Rank::Contains)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::configuration::{Capabilities, Validate};

    use super::*;

    fn capabilities(profiles: u8) -> Capabilities {
        Capabilities {
            key_rows: 1,
            key_columns: 1,
            profiles,
            layouts: openwoot_protocol::heapless::Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 0,
            edition: None,
        }
    }

    #[test]
    fn every_offered_action_is_accepted_by_the_firmware() {
        let capabilities = capabilities(4);
        for entry in entries(capabilities.profiles) {
            assert_eq!(
                entry.action.validate(&capabilities),
                Ok(()),
                "{:?} is refused",
                entry.action
            );
        }
    }

    #[test]
    fn every_category_offers_something() {
        let offered = entries(4);
        for category in Category::ALL {
            assert!(
                offered.iter().any(|entry| entry.category == category),
                "{category:?} is empty"
            );
        }
    }

    #[test]
    fn profile_slots_follow_the_capabilities() {
        let profiles = |slots| {
            entries(slots)
                .into_iter()
                .filter(|entry| matches!(entry.action, Action::Function(Function::Profile(_))))
                .count()
        };
        assert_eq!(profiles(2), 2);
        assert_eq!(profiles(4), 4);
    }

    #[test]
    fn letters_are_basic_and_modifiers_extended() {
        assert_eq!(keyboard_category(0x04), Category::Basic);
        assert_eq!(keyboard_category(0x38), Category::Basic);
        assert_eq!(keyboard_category(0x64), Category::Basic);
        assert_eq!(keyboard_category(0x2C), Category::Extended);
        assert_eq!(keyboard_category(0xE1), Category::Extended);
        assert_eq!(keyboard_category(0x88), Category::International);
        assert_eq!(keyboard_category(0x7C), Category::OtherKeys);
    }

    #[test]
    fn exact_matches_come_before_partial_ones() {
        let found = search(&entries(4), "f1");
        assert_eq!(
            found.first().map(|entry| entry.action),
            Some(Action::Keyboard(0x3A))
        );
        assert!(
            found
                .iter()
                .any(|entry| entry.action == Action::Keyboard(0x43))
        );
        let volume = search(&entries(4), "volume up");
        assert!(
            volume
                .iter()
                .any(|entry| entry.action == Action::Consumer(0x00E9))
        );
        assert_eq!(search(&entries(4), "zzzz"), Vec::new());
    }

    #[test]
    fn a_key_cannot_open_its_own_layer_and_layer_lock_needs_a_fn_layer() {
        assert!(refusal(Action::Layer(Layer::One), Layer::One).is_some());
        assert!(refusal(Action::Layer(Layer::One), Layer::Base).is_none());
        assert!(refusal(Action::Layer(Layer::Two), Layer::One).is_none());
        assert!(refusal(Action::Layer(Layer::Base), Layer::Two).is_some());
        assert!(refusal(Action::Function(Function::LayerLock), Layer::Base).is_some());
        assert!(refusal(Action::Function(Function::LayerLock), Layer::Three).is_none());
        assert!(refusal(Action::Keyboard(0x04), Layer::Base).is_none());
    }
}
