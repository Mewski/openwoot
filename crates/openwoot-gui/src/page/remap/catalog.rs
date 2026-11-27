use gpui_kit::SharedString;
use openwoot_protocol::{
    keys::{Action, Function, Layer},
    profile::Slot,
};

use crate::usage;

const BASIC: &[u8] = &[
    0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13,
    0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x22, 0x23,
    0x24, 0x25, 0x26, 0x27, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x64,
];
const EXTENDED: &[u8] = &[
    0x29, 0x2B, 0x39, 0x2A, 0x28, 0x2C, 0xE0, 0xE4, 0xE1, 0xE5, 0xE2, 0xE6, 0xE3, 0xE7, 0x65, 0x90,
    0x91, 0x92, 0x93, 0x94, 0x87, 0x88, 0x89, 0x8A, 0x8B, 0x8C, 0x4F, 0x50, 0x51, 0x52, 0x46, 0x47,
    0x48, 0x4A, 0x4D, 0x49, 0x4C, 0x4B, 0x4E, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F, 0x40, 0x41, 0x42,
    0x43, 0x44, 0x45, 0x68, 0x69, 0x6A, 0x6B, 0x6C, 0x6D, 0x6E, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x53,
    0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x61, 0x62, 0x54, 0x63, 0x55, 0x56, 0x57, 0x58,
];
const LAUNCHERS: &[u16] = &[
    0x0183, 0x018A, 0x0192, 0x0194, 0x0221, 0x0223, 0x0224, 0x0225, 0x0227, 0x022A,
];
const DESKTOP: &[u16] = &[0x029F, 0x02A0, 0x0070, 0x006F];
const MEDIA: &[u16] = &[0x00B6, 0x00CD, 0x00B7, 0x00B5, 0x00E2, 0x00EA, 0x00E9];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Category {
    Basic,
    Extended,
    Functions,
    Profiles,
    Macros,
    Media,
    Mouse,
    Gamepad,
    OtherKeys,
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
    pub(crate) const ALL: [Self; 9] = [
        Self::Basic,
        Self::Extended,
        Self::Functions,
        Self::Profiles,
        Self::Macros,
        Self::Media,
        Self::Mouse,
        Self::Gamepad,
        Self::OtherKeys,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Basic => "Basic Characters",
            Self::Extended => "Extended Characters",
            Self::Functions => "Functions",
            Self::Profiles => "Profiles",
            Self::Macros => "Macros",
            Self::Media => "Media and Audio control",
            Self::Mouse => "Mouse",
            Self::Gamepad => "Gamepad",
            Self::OtherKeys => "Other Keys",
        }
    }
}

pub(crate) fn entries(profile_slots: u8, macros: impl IntoIterator<Item = u8>) -> Vec<Entry> {
    let entry = |category| move |action| Entry { action, category };
    let basic = BASIC
        .iter()
        .map(|code| Action::Keyboard(*code))
        .map(entry(Category::Basic));
    let extended = EXTENDED
        .iter()
        .map(|code| Action::Keyboard(*code))
        .map(entry(Category::Extended));
    let functions = LAUNCHERS
        .iter()
        .map(|code| Action::Consumer(*code))
        .chain(
            usage::SYSTEM
                .iter()
                .map(|(control, _, _)| Action::System(*control)),
        )
        .chain(DESKTOP.iter().map(|code| Action::Consumer(*code)))
        .chain(
            [
                Function::BrightnessUp,
                Function::BrightnessDown,
                Function::ToggleKeyDisable,
            ]
            .map(Action::Function),
        )
        .chain([Layer::One, Layer::Two, Layer::Three].map(Action::Layer))
        .chain([Action::Function(Function::LayerLock)])
        .map(entry(Category::Functions));
    let profiles = (0..profile_slots)
        .map(|slot| Action::Function(Function::Profile(Slot::new(slot))))
        .chain([
            Action::Function(Function::ToggleModeProfile),
            Action::Function(Function::NextProfile),
        ])
        .map(entry(Category::Profiles));
    let macros = macros
        .into_iter()
        .map(Action::Macro)
        .map(entry(Category::Macros));
    let media = MEDIA
        .iter()
        .map(|code| Action::Consumer(*code))
        .map(entry(Category::Media));
    let mouse = usage::MOUSE
        .iter()
        .map(|(button, _, _)| Action::Mouse(*button))
        .map(entry(Category::Mouse));
    let gamepad = usage::GAMEPAD
        .iter()
        .map(|(control, _, _)| Action::Gamepad(*control))
        .map(entry(Category::Gamepad));
    let other_keys = usage::KEYBOARD
        .iter()
        .map(|(code, _, _)| *code)
        .filter(|code| !BASIC.contains(code) && !EXTENDED.contains(code))
        .map(Action::Keyboard)
        .chain(
            usage::CONSUMER
                .iter()
                .map(|(code, _, _)| *code)
                .filter(|code| {
                    !LAUNCHERS.contains(code) && !DESKTOP.contains(code) && !MEDIA.contains(code)
                })
                .map(Action::Consumer),
        )
        .map(entry(Category::OtherKeys));
    basic
        .chain(extended)
        .chain(functions)
        .chain(profiles)
        .chain(macros)
        .chain(media)
        .chain(mouse)
        .chain(gamepad)
        .chain(other_keys)
        .collect()
}

pub(crate) fn search(
    entries: &[Entry],
    query: &str,
    named: impl Fn(Action) -> Option<SharedString>,
) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return entries.to_vec();
    }
    let mut ranked: Vec<(Category, Rank, usize, Entry)> = entries
        .iter()
        .enumerate()
        .filter_map(|(position, entry)| {
            rank(entry.action, named(entry.action).as_deref(), &query)
                .map(|rank| (entry.category, rank, position, *entry))
        })
        .collect();
    ranked.sort_by_key(|(category, rank, position, _)| (*category, *rank, *position));
    ranked.into_iter().map(|(_, _, _, entry)| entry).collect()
}

pub(crate) fn refusal(action: Action, layer: Layer) -> Option<String> {
    match action {
        Action::Layer(Layer::Base) => {
            Some("The Main Layer can't be held like a Fn layer".to_owned())
        }
        Action::Layer(opened) if opened == layer => Some(format!(
            "{} key can't be added to the same layer it activates",
            usage::legend(action)
        )),
        Action::Function(Function::LayerLock) if layer == Layer::Base => Some(format!(
            "{} key can't be added to the first layer",
            usage::legend(action)
        )),
        _ => None,
    }
}

pub(crate) fn help(action: Action, name: Option<&str>) -> Option<String> {
    Some(match action {
        Action::Layer(layer) => format!(
            "While pressed, access Fn layer {} mapped keys.",
            layer.number()
        ),
        Action::Function(Function::LayerLock) => {
            "Keeps this Fn layer on until pressed again.".to_owned()
        }
        Action::Function(Function::ToggleKeyDisable) => {
            "Place this in the Fn layer to disable the same key in the Main Layer.".to_owned()
        }
        Action::Function(Function::ToggleModeProfile) => {
            "Toggle between the default profile and the last used onboard profile.".to_owned()
        }
        Action::Function(Function::NextProfile) => "Cycle through all active profiles.".to_owned(),
        Action::Function(Function::Profile(_)) => match name {
            Some(name) => format!("{}: {name}", usage::name(action)),
            None => usage::name(action).into_owned(),
        },
        Action::Macro(_) => match name {
            Some(name) => format!("{}: {name}", usage::name(action)),
            None => format!("{} is empty", usage::name(action)),
        },
        Action::Consumer(_)
        | Action::System(_)
        | Action::Mouse(_)
        | Action::Gamepad(_)
        | Action::Function(_) => capitalized(&usage::name(action)),
        Action::Keyboard(_) | Action::None => return None,
    })
}

pub(crate) fn capitalized(text: &str) -> String {
    let mut characters = text.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(characters).collect()
    })
}

fn rank(action: Action, named: Option<&str>, query: &str) -> Option<Rank> {
    let short = usage::legend(action).to_lowercase();
    let mut long = vec![usage::name(action).to_lowercase()];
    long.extend(named.map(str::to_lowercase));
    let worded = |text: &String| {
        text.split(|character: char| character.is_whitespace() || character == '/')
            .any(|word| word.starts_with(query))
    };
    if short == query || long.iter().any(|text| text == query) {
        Some(Rank::Exact)
    } else if short.starts_with(query) || long.iter().any(worded) {
        Some(Rank::Prefix)
    } else if short.contains(query) || long.iter().any(|text| text.contains(query)) {
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
            linked_profiles: 0,
            layouts: openwoot_protocol::heapless::Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 0,
            edition: None,
        }
    }

    const MACROS: [u8; 2] = [0, 5];

    fn offered(profiles: u8) -> Vec<Entry> {
        entries(profiles, MACROS)
    }

    fn unnamed(_: Action) -> Option<SharedString> {
        None
    }

    #[test]
    fn every_offered_action_is_accepted_by_the_firmware() {
        let capabilities = capabilities(4);
        for entry in offered(capabilities.profiles) {
            assert_eq!(
                entry.action.validate(&capabilities),
                Ok(()),
                "{:?} is refused",
                entry.action
            );
        }
    }

    #[test]
    fn every_category_offers_something_and_nothing_is_offered_twice() {
        let offered = offered(4);
        for category in Category::ALL {
            assert!(
                offered.iter().any(|entry| entry.category == category),
                "{category:?} is empty"
            );
        }
        for (position, entry) in offered.iter().enumerate() {
            assert!(
                offered
                    .iter()
                    .skip(position + 1)
                    .all(|other| other.action != entry.action),
                "{:?} is offered twice",
                entry.action
            );
        }
    }

    #[test]
    fn every_named_usage_is_offered() {
        let offered = offered(4);
        for (code, _, _) in usage::KEYBOARD {
            assert!(
                offered
                    .iter()
                    .any(|entry| entry.action == Action::Keyboard(*code)),
                "usage {code:#04x} is missing"
            );
        }
        for (code, _, _) in usage::CONSUMER {
            assert!(
                offered
                    .iter()
                    .any(|entry| entry.action == Action::Consumer(*code)),
                "consumer usage {code:#06x} is missing"
            );
        }
    }

    #[test]
    fn profile_slots_follow_the_capabilities() {
        let profiles = |slots| {
            entries(slots, [])
                .into_iter()
                .filter(|entry| matches!(entry.action, Action::Function(Function::Profile(_))))
                .count()
        };
        assert_eq!(profiles(2), 2);
        assert_eq!(profiles(4), 4);
    }

    #[test]
    fn categories_follow_wootility() {
        let category = |action| {
            offered(4)
                .into_iter()
                .find(|entry| entry.action == action)
                .map(|entry| entry.category)
        };
        assert_eq!(category(Action::Keyboard(0x04)), Some(Category::Basic));
        assert_eq!(category(Action::Keyboard(0x64)), Some(Category::Basic));
        assert_eq!(category(Action::Keyboard(0xE1)), Some(Category::Extended));
        assert_eq!(category(Action::Keyboard(0x58)), Some(Category::Extended));
        assert_eq!(
            category(Action::Consumer(0x0192)),
            Some(Category::Functions)
        );
        assert_eq!(
            category(Action::Layer(Layer::Two)),
            Some(Category::Functions)
        );
        assert_eq!(category(Action::Consumer(0x00E9)), Some(Category::Media));
        assert_eq!(category(Action::Macro(5)), Some(Category::Macros));
        assert_eq!(category(Action::Keyboard(0x7C)), Some(Category::OtherKeys));
    }

    #[test]
    fn only_stored_macros_are_offered() {
        let macros: Vec<Action> = offered(4)
            .into_iter()
            .filter(|entry| entry.category == Category::Macros)
            .map(|entry| entry.action)
            .collect();
        assert_eq!(macros, [Action::Macro(0), Action::Macro(5)]);
    }

    #[test]
    fn search_groups_results_by_category_and_ranks_inside_each() {
        let found = search(&offered(4), "f1", unnamed);
        assert_eq!(
            found.first().map(|entry| entry.action),
            Some(Action::Keyboard(0x3A))
        );
        assert!(
            found
                .iter()
                .any(|entry| entry.action == Action::Keyboard(0x43))
        );
        let volume = search(&offered(4), "volume up", unnamed);
        assert!(
            volume
                .iter()
                .any(|entry| entry.action == Action::Consumer(0x00E9))
        );
        let categories: Vec<Category> = search(&offered(4), "a", unnamed)
            .iter()
            .map(|entry| entry.category)
            .collect();
        assert!(
            categories
                .windows(2)
                .all(|pair| pair.first() <= pair.last())
        );
        assert_eq!(search(&offered(4), "zzzz", unnamed), Vec::new());
    }

    #[test]
    fn macros_are_found_by_name_and_by_number() {
        let named = |action| match action {
            Action::Macro(5) => Some(SharedString::from("Copy line")),
            _ => None,
        };
        let found = |query| {
            search(&offered(4), query, named)
                .into_iter()
                .map(|entry| entry.action)
                .collect::<Vec<_>>()
        };
        assert_eq!(found("line"), [Action::Macro(5)]);
        assert_eq!(found("macro 1"), [Action::Macro(0)]);
        assert!(found("macro").contains(&Action::Macro(0)));
    }

    #[test]
    fn macro_help_names_the_macro_or_its_empty_slot() {
        assert_eq!(
            help(Action::Macro(2), Some("Greeting")).as_deref(),
            Some("Macro 3: Greeting")
        );
        assert_eq!(
            help(Action::Macro(2), None).as_deref(),
            Some("Macro 3 is empty")
        );
    }

    #[test]
    fn a_key_cannot_open_its_own_layer_and_fn_lock_needs_a_fn_layer() {
        assert!(refusal(Action::Layer(Layer::One), Layer::One).is_some());
        assert!(refusal(Action::Layer(Layer::One), Layer::Base).is_none());
        assert!(refusal(Action::Layer(Layer::Two), Layer::One).is_none());
        assert!(refusal(Action::Layer(Layer::Base), Layer::Two).is_some());
        assert!(refusal(Action::Function(Function::LayerLock), Layer::Base).is_some());
        assert!(refusal(Action::Function(Function::LayerLock), Layer::Three).is_none());
        assert!(refusal(Action::Keyboard(0x04), Layer::Base).is_none());
    }
}
