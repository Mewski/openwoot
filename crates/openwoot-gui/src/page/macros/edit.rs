use openwoot_protocol::macros::{Macro, NAME_LEN, Name, Step, Steps};

pub(super) fn created(index: u8) -> Macro {
    Macro {
        name: fitted(&format!("Macro {}", u16::from(index) + 1)),
        ..Macro::default()
    }
}

pub(super) fn duplicated(source: &Macro) -> Macro {
    Macro {
        name: fitted(&format!("{} copy", source.name)),
        ..source.clone()
    }
}

pub(super) fn named(typed: &str) -> Option<Name> {
    let trimmed = typed.trim();
    (!trimmed.is_empty()).then(|| fitted(trimmed))
}

pub(super) fn insert(steps: &mut Steps, at: usize, new: &[Step]) -> usize {
    let at = at.min(steps.len());
    let room = steps.capacity() - steps.len();
    let inserted = new.len().min(room);
    for (offset, step) in new.iter().take(inserted).enumerate() {
        if steps.insert(at + offset, step.clone()).is_err() {
            return offset;
        }
    }
    inserted
}

pub(super) fn remove(steps: &mut Steps, index: usize) -> bool {
    if index < steps.len() {
        steps.remove(index);
        true
    } else {
        false
    }
}

pub(super) fn reorder(steps: &mut Steps, from: usize, to: usize) -> bool {
    if from == to || from >= steps.len() || to >= steps.len() {
        return false;
    }
    let moved = steps.remove(from);
    steps.insert(to, moved).is_ok()
}

fn fitted(text: &str) -> Name {
    let mut name = Name::new();
    for character in text.chars() {
        if name.len() + character.len_utf8() > NAME_LEN || name.push(character).is_err() {
            break;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        keys::Action,
        macros::{MAX_STEPS, Mode},
    };

    use super::*;

    fn delays(values: &[u16]) -> Steps {
        values.iter().map(|value| Step::Delay(*value)).collect()
    }

    fn values(steps: &Steps) -> Vec<u16> {
        steps
            .iter()
            .filter_map(|step| match step {
                Step::Delay(value) => Some(*value),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_new_macro_is_named_after_its_slot_and_plays_once() {
        let created = created(4);
        assert_eq!(created.name.as_str(), "Macro 5");
        assert_eq!(created.mode, Mode::Once);
        assert!(created.steps.is_empty());
    }

    #[test]
    fn a_duplicate_keeps_the_steps_and_mode_under_a_new_name() {
        let source = Macro {
            name: Name::try_from("Paste").unwrap(),
            mode: Mode::Toggle,
            steps: [Step::Tap(Action::Keyboard(0x19))].into_iter().collect(),
        };
        let copy = duplicated(&source);
        assert_eq!(copy.name.as_str(), "Paste copy");
        assert_eq!(copy.mode, Mode::Toggle);
        assert_eq!(copy.steps, source.steps);
        let long = Macro {
            name: Name::try_from("A rather long macro name of 32 b").unwrap(),
            ..source
        };
        assert_eq!(
            duplicated(&long).name.as_str(),
            "A rather long macro name of 32 b"
        );
    }

    #[test]
    fn names_are_trimmed_fitted_and_never_empty() {
        assert_eq!(named("  Greeting  ").as_deref(), Some("Greeting"));
        assert_eq!(named("   "), None);
        let fitted = named(&"é".repeat(20)).unwrap();
        assert_eq!(fitted.chars().count(), 16);
        assert!(fitted.len() <= NAME_LEN);
    }

    #[test]
    fn steps_are_inserted_where_asked_until_the_macro_is_full() {
        let mut steps = delays(&[1, 2, 3]);
        assert_eq!(insert(&mut steps, 1, &delays(&[7, 8])), 2);
        assert_eq!(values(&steps), [1, 7, 8, 2, 3]);
        assert_eq!(insert(&mut steps, 99, &delays(&[9])), 1);
        assert_eq!(values(&steps), [1, 7, 8, 2, 3, 9]);
        let mut full: Steps = (0..MAX_STEPS - 1).map(|_| Step::Delay(1)).collect();
        assert_eq!(insert(&mut full, 0, &delays(&[5, 6])), 1);
        assert_eq!(full.first(), Some(&Step::Delay(5)));
        assert_eq!(full.len(), MAX_STEPS);
    }

    #[test]
    fn steps_are_removed_and_reordered_within_the_list() {
        let mut steps = delays(&[1, 2, 3, 4]);
        assert!(reorder(&mut steps, 0, 2));
        assert_eq!(values(&steps), [2, 3, 1, 4]);
        assert!(reorder(&mut steps, 3, 0));
        assert_eq!(values(&steps), [4, 2, 3, 1]);
        assert!(!reorder(&mut steps, 1, 1));
        assert!(!reorder(&mut steps, 1, 4));
        assert!(remove(&mut steps, 1));
        assert_eq!(values(&steps), [4, 3, 1]);
        assert!(!remove(&mut steps, 3));
    }
}
