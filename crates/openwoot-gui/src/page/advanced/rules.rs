use openwoot_protocol::{
    advanced::{self, Behaviour, MAX_ENTRIES},
    configuration::{Capabilities, Invalid, Validate as _},
    heapless,
    keys::{Index, Layer},
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum Problem {
    #[error("a profile holds at most {MAX_ENTRIES} advanced keys")]
    TooMany,
    #[error("{key} already has an advanced key on {layer}")]
    BoundTwice { key: String, layer: &'static str },
    #[error("{key} cannot be paired with itself")]
    PairedWithItself { key: String },
    #[error("the hold duration of {key} is zero")]
    ZeroHoldTime { key: String },
    #[error(transparent)]
    Invalid(#[from] Invalid),
}

pub(crate) fn other_key(entry: &advanced::Key) -> Option<Index> {
    match entry.behaviour {
        Behaviour::DepthPriority(pair) => Some(pair.other),
        Behaviour::Socd(pair) => Some(pair.other),
        Behaviour::DynamicKeystroke(_) | Behaviour::ModTap(_) | Behaviour::Toggle(_) => None,
    }
}

pub(crate) fn bindings(entry: &advanced::Key) -> impl Iterator<Item = (Index, Layer)> + use<> {
    let layer = entry.layer;
    core::iter::once(entry.key)
        .chain(other_key(entry))
        .map(move |key| (key, layer))
}

pub(crate) fn holder(entries: &[advanced::Key], key: Index, layer: Layer) -> Option<usize> {
    entries
        .iter()
        .position(|entry| bindings(entry).any(|binding| binding == (key, layer)))
}

pub(crate) fn check(
    entries: &[advanced::Key],
    capabilities: &Capabilities,
    name: impl Fn(Index) -> String,
    layer_name: impl Fn(Layer) -> &'static str,
) -> Result<(), Problem> {
    if entries.len() > MAX_ENTRIES {
        return Err(Problem::TooMany);
    }
    let mut seen: Vec<(Index, Layer)> = Vec::with_capacity(entries.len() * 2);
    for entry in entries {
        if other_key(entry) == Some(entry.key) {
            return Err(Problem::PairedWithItself {
                key: name(entry.key),
            });
        }
        if let Behaviour::ModTap(mod_tap) = entry.behaviour
            && mod_tap.hold_time_ms == 0
        {
            return Err(Problem::ZeroHoldTime {
                key: name(entry.key),
            });
        }
        for binding in bindings(entry) {
            if seen.contains(&binding) {
                return Err(Problem::BoundTwice {
                    key: name(binding.0),
                    layer: layer_name(binding.1),
                });
            }
            seen.push(binding);
        }
    }
    let keys: advanced::Keys = heapless::Vec::from_slice(entries).map_err(|_| Problem::TooMany)?;
    keys.validate(capabilities)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        advanced::{Interrupt, ModTap, Resolution, Socd},
        keys::Action,
    };

    use super::*;

    fn capabilities() -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            linked_profiles: 0,
            layouts: heapless::Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 0,
            edition: None,
        }
    }

    fn mod_tap(key: u8, layer: Layer, hold_time_ms: u16) -> advanced::Key {
        advanced::Key {
            key: Index::new(key),
            layer,
            behaviour: Behaviour::ModTap(ModTap {
                tap: Action::Keyboard(0x29),
                hold: Action::Keyboard(0xE0),
                hold_time_ms,
                interrupt: Interrupt::Auto,
            }),
        }
    }

    fn socd(key: u8, other: u8) -> advanced::Key {
        advanced::Key {
            key: Index::new(key),
            layer: Layer::Base,
            behaviour: Behaviour::Socd(Socd {
                other: Index::new(other),
                resolution: Resolution::LastPressed,
                both_when_bottomed_out: false,
            }),
        }
    }

    fn verdict(entries: &[advanced::Key]) -> Result<(), Problem> {
        check(
            entries,
            &capabilities(),
            |key| format!("key {}", key.get()),
            |layer| match layer {
                Layer::Base => "the base layer",
                Layer::One | Layer::Two | Layer::Three => "a Fn layer",
            },
        )
    }

    #[test]
    fn a_key_takes_one_advanced_key_per_layer() {
        assert_eq!(
            verdict(&[mod_tap(3, Layer::Base, 200), mod_tap(3, Layer::One, 200)]),
            Ok(())
        );
        assert_eq!(
            verdict(&[mod_tap(3, Layer::Base, 200), mod_tap(3, Layer::Base, 150)]),
            Err(Problem::BoundTwice {
                key: "key 3".to_owned(),
                layer: "the base layer",
            })
        );
    }

    #[test]
    fn a_pair_binds_both_of_its_keys() {
        assert_eq!(
            verdict(&[socd(10, 12), mod_tap(12, Layer::Base, 200)]),
            Err(Problem::BoundTwice {
                key: "key 12".to_owned(),
                layer: "the base layer",
            })
        );
        assert_eq!(
            verdict(&[socd(10, 10)]),
            Err(Problem::PairedWithItself {
                key: "key 10".to_owned(),
            })
        );
    }

    #[test]
    fn a_mod_tap_needs_a_hold_duration() {
        assert_eq!(
            verdict(&[mod_tap(3, Layer::Base, 0)]),
            Err(Problem::ZeroHoldTime {
                key: "key 3".to_owned(),
            })
        );
    }

    #[test]
    fn a_profile_holds_at_most_forty_advanced_keys() {
        let entries: Vec<advanced::Key> =
            (0..=40).map(|key| mod_tap(key, Layer::Base, 200)).collect();
        assert_eq!(verdict(&entries), Err(Problem::TooMany));
        assert_eq!(verdict(entries.get(..40).unwrap_or_default()), Ok(()));
    }

    #[test]
    fn the_firmware_rules_still_apply() {
        assert_eq!(
            verdict(&[mod_tap(200, Layer::Base, 200)]),
            Err(Problem::Invalid(Invalid::KeyOutsideMatrix(200)))
        );
    }

    #[test]
    fn holders_are_found_through_both_keys_of_a_pair() {
        let entries = [mod_tap(1, Layer::Base, 200), socd(10, 12)];
        assert_eq!(holder(&entries, Index::new(12), Layer::Base), Some(1));
        assert_eq!(holder(&entries, Index::new(1), Layer::Base), Some(0));
        assert_eq!(holder(&entries, Index::new(1), Layer::Two), None);
    }
}
