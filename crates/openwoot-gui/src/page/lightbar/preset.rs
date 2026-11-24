use openwoot_protocol::lightbar::{
    Builtin, Comparator, Effect, Entry, Range, Signal, Solid, Target, Term,
};

use super::entry::{DEFAULT_TIME_MS, Kind, Matrix, color, compare, full_range, meter, single};

const KEY_DEPTH_THRESHOLD: f32 = 5.0;
const CAPS_LOCK_LEDS: u8 = 2;

pub(super) struct Preset {
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    build: fn(Matrix) -> Entry,
}

pub(super) const PRESETS: [Preset; 8] = [
    Preset {
        name: "Solid",
        description: "Lights the light bar in one colour.",
        build: |matrix| plain(Kind::Solid.effect(matrix)),
    },
    Preset {
        name: "Pulse",
        description: "Fades a colour in and out.",
        build: |matrix| plain(Kind::Blink.effect(matrix)),
    },
    Preset {
        name: "Sync with Keyboard",
        description: "Matches the light bar with the colours of the number row.",
        build: |matrix| plain(Kind::Mirror.effect(matrix)),
    },
    Preset {
        name: "Profile Indicator",
        description: "Shows the colour and slot of the active profile.",
        build: |matrix| plain(Kind::ProfileIndicator.effect(matrix)),
    },
    Preset {
        name: "Caps Lock Indicator",
        description: "Lights the right end of the light bar while Caps Lock is on.",
        build: caps_lock,
    },
    Preset {
        name: "Brightness Change",
        description: "Shows the brightness for a moment whenever it changes.",
        build: brightness,
    },
    Preset {
        name: "Key Depth",
        description: "Shows how deep the deepest key is pressed, as a bar.",
        build: key_depth,
    },
    Preset {
        name: "Typing Speed Meter (KPS)",
        description: "More LEDs light up the more keys are pressed per second.",
        build: typing_speed,
    },
];

impl Preset {
    pub(super) fn entry(&self, matrix: Matrix) -> Entry {
        (self.build)(matrix)
    }
}

const fn plain(effect: Effect) -> Entry {
    Entry {
        condition: None,
        effect,
    }
}

fn caps_lock(matrix: Matrix) -> Entry {
    let last = matrix.leds.saturating_sub(1);
    Entry {
        condition: Some(single(compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::Equal,
            1.0,
        ))),
        effect: Effect::Solid(Solid {
            color: color(255, 255, 255),
            target: Target::Lightbar(Range {
                first: matrix.leds.saturating_sub(CAPS_LOCK_LEDS),
                last,
            }),
        }),
    }
}

fn brightness(matrix: Matrix) -> Entry {
    Entry {
        condition: Some(single(Term::Trigger {
            signal: Signal::Builtin(Builtin::Brightness),
            duration_ms: DEFAULT_TIME_MS,
        })),
        effect: Effect::Meter(meter(
            Builtin::Brightness,
            color(255, 255, 255),
            None,
            full_range(matrix.leds),
        )),
    }
}

fn key_depth(matrix: Matrix) -> Entry {
    Entry {
        condition: Some(single(compare(
            Signal::Builtin(Builtin::AnalogLevel),
            Comparator::Greater,
            KEY_DEPTH_THRESHOLD,
        ))),
        effect: Effect::Meter(meter(
            Builtin::AnalogLevel,
            color(255, 0, 0),
            None,
            full_range(matrix.leds),
        )),
    }
}

fn typing_speed(matrix: Matrix) -> Entry {
    plain(Effect::Meter(meter(
        Builtin::KeyRate,
        color(255, 0, 0),
        Some(color(0, 255, 0)),
        full_range(matrix.leds),
    )))
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        configuration::{Capabilities, Validate as _},
        heapless,
        lightbar::{Entries, Meter},
    };

    use super::*;
    use crate::page::lightbar::entry::maximum_of;

    fn capabilities(leds: u8) -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            layouts: heapless::Vec::new(),
            lightbar_leds: leds,
            host_signals: false,
            switch_models: 1,
            edition: None,
        }
    }

    #[test]
    fn every_preset_is_valid_on_every_light_bar() {
        for leds in [1, 2, 10, 16] {
            let matrix = Matrix {
                leds,
                rows: 6,
                columns: 17,
            };
            for preset in &PRESETS {
                let entry = preset.entry(matrix);
                assert_eq!(
                    entry.validate(&capabilities(leds)),
                    Ok(()),
                    "{}",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn the_caps_lock_indicator_uses_the_last_two_leds() {
        let entry = caps_lock(Matrix {
            leds: 10,
            rows: 6,
            columns: 17,
        });
        assert!(matches!(
            entry.effect,
            Effect::Solid(Solid {
                target: Target::Lightbar(Range { first: 8, last: 9 }),
                ..
            })
        ));
    }

    #[test]
    fn the_meters_follow_their_signal_scale() {
        let matrix = Matrix {
            leds: 10,
            rows: 6,
            columns: 17,
        };
        let maxima: Vec<f32> = [brightness(matrix), key_depth(matrix), typing_speed(matrix)]
            .iter()
            .filter_map(|entry| match &entry.effect {
                Effect::Meter(Meter { maximum, .. }) => Some(*maximum),
                _ => None,
            })
            .collect();
        assert_eq!(maxima, [100.0, 255.0, 30.0]);
        let entries: Entries = [brightness(matrix), key_depth(matrix)]
            .into_iter()
            .collect();
        assert_eq!(entries.validate(&capabilities(10)), Ok(()));
        assert!(maximum_of(Signal::Builtin(Builtin::KeyRate)) > 0.0);
    }
}
