use std::{
    fs, io,
    path::{Path, PathBuf},
};

use openwoot_protocol::{PROTOCOL_VERSION, profile};
use serde::{Deserialize, Serialize};

use super::parts::Parts;

pub(super) const FORMAT: u16 = 1;
pub(super) const EXTENSION: &str = "json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct ProfileFile {
    pub(super) format: u16,
    pub(super) protocol: u16,
    pub(super) keyboard: String,
    pub(super) info: profile::Info,
    pub(super) parts: Parts,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum FileError {
    #[error("could not read `{}`", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not write `{}`", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("`{}` is not an openwoot profile", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("could not encode the profile")]
    Encode(#[source] serde_json::Error),
    #[error(
        "`{}` holds profile format {found}; this version of openwoot reads format {FORMAT}",
        path.display()
    )]
    UnknownFormat { path: PathBuf, found: u16 },
}

#[derive(Deserialize)]
struct Header {
    format: u16,
}

impl ProfileFile {
    pub(super) fn new(keyboard: String, info: profile::Info, parts: Parts) -> Self {
        Self {
            format: FORMAT,
            protocol: PROTOCOL_VERSION,
            keyboard,
            info,
            parts,
        }
    }

    pub(super) fn load(path: &Path) -> Result<Self, FileError> {
        let text = fs::read_to_string(path).map_err(|source| FileError::Read {
            path: path.to_owned(),
            source,
        })?;
        Self::parse(path, &text)
    }

    pub(super) fn save(&self, path: &Path) -> Result<(), FileError> {
        let text = self.encode()?;
        fs::write(path, text).map_err(|source| FileError::Write {
            path: path.to_owned(),
            source,
        })
    }

    fn parse(path: &Path, text: &str) -> Result<Self, FileError> {
        let parse_error = |source| FileError::Parse {
            path: path.to_owned(),
            source,
        };
        let header: Header = serde_json::from_str(text).map_err(parse_error)?;
        if header.format != FORMAT {
            return Err(FileError::UnknownFormat {
                path: path.to_owned(),
                found: header.format,
            });
        }
        serde_json::from_str(text).map_err(parse_error)
    }

    fn encode(&self) -> Result<String, FileError> {
        serde_json::to_string_pretty(self).map_err(FileError::Encode)
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        advanced,
        configuration::{Capabilities, Invalid},
        gamepad::{self, CurvePoint, Normalization},
        heapless,
        keys::{self, Action, Distance, RapidTrigger, ScanMode, TapDuration},
        lighting::{self, Color, Indicators, Rgb},
    };

    use super::*;

    const KEYS: usize = 6;

    fn capabilities(keys: u8) -> Capabilities {
        Capabilities {
            key_rows: 1,
            key_columns: keys,
            profiles: 4,
            layouts: heapless::Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 1,
            edition: None,
        }
    }

    fn filled<T: Clone, const N: usize>(value: &T) -> heapless::Vec<T, N> {
        let mut entries = heapless::Vec::new();
        for _ in 0..KEYS {
            assert!(entries.push(value.clone()).is_ok());
        }
        entries
    }

    fn sample() -> ProfileFile {
        let keymap: keys::Keymap = filled(&Action::Keyboard(0x04));
        let parts = Parts {
            key_settings: keys::Settings {
                actuation: Distance::from_fraction(20_000),
                rapid_trigger_enabled: true,
                rapid_trigger: RapidTrigger {
                    press: Distance::from_fraction(3_000),
                    release: None,
                    continuous: false,
                },
                tap_duration: TapDuration::FortyMilliseconds,
                scan_mode: ScanMode::Parallel,
                keyboard_in_gamepad_mode: false,
            },
            keymaps: [keymap.clone(), keymap.clone(), keymap.clone(), keymap],
            overrides: filled(&keys::Override::default()),
            advanced_keys: advanced::Keys::new(),
            gamepad_mapping: gamepad::Mapping {
                bindings: filled(&None),
                exclusive: false,
            },
            gamepad_settings: gamepad::Settings {
                curve: [
                    CurvePoint {
                        travel: 10,
                        output: 0,
                    },
                    CurvePoint {
                        travel: 60,
                        output: 40,
                    },
                    CurvePoint {
                        travel: 160,
                        output: 120,
                    },
                    CurvePoint {
                        travel: 240,
                        output: 255,
                    },
                ],
                normalization: Normalization::Circular,
                single_direction_axes: false,
                left_stick_up_bias: None,
            },
            lighting: lighting::Settings {
                brightness: 200,
                indicators: Indicators {
                    caps_lock: Color::TRANSPARENT,
                    scroll_lock: Color::TRANSPARENT,
                    num_lock: Color::TRANSPARENT,
                    layer_lock: Color::TRANSPARENT,
                },
                effects: heapless::Vec::new(),
            },
            key_colors: [Some(filled(&Rgb::default())), None, None, None],
            lightbar: None,
        };
        let mut name = heapless::String::new();
        assert!(name.push_str("Rapid").is_ok());
        ProfileFile::new(
            "Wooting 80HE".to_owned(),
            profile::Info {
                name,
                color: Rgb {
                    red: 255,
                    green: 40,
                    blue: 40,
                },
            },
            parts,
        )
    }

    #[test]
    fn a_saved_profile_reads_back_unchanged() {
        let file = sample();
        let text = file.encode().unwrap_or_default();
        let read = ProfileFile::parse(Path::new("rapid.json"), &text).ok();
        assert_eq!(read, Some(file));
    }

    #[test]
    fn another_format_is_refused() {
        let text = r#"{ "format": 9, "anything": true }"#;
        assert!(matches!(
            ProfileFile::parse(Path::new("future.json"), text),
            Err(FileError::UnknownFormat { found: 9, .. })
        ));
    }

    #[test]
    fn a_file_that_is_no_profile_is_refused() {
        assert!(matches!(
            ProfileFile::parse(Path::new("notes.json"), r#"{ "format": 1 }"#),
            Err(FileError::Parse { .. })
        ));
    }

    #[test]
    fn parts_fit_only_a_keyboard_with_the_same_key_count() {
        let parts = sample().parts;
        assert!(parts.check(&capabilities(6)).is_ok());
        let mismatch = parts
            .check(&capabilities(8))
            .err()
            .map(|error| error.to_string());
        assert_eq!(
            mismatch,
            Some(format!(
                "could not take over the key bindings: {}",
                Invalid::KeyCountDiffersFromMatrix {
                    expected: 8,
                    found: 6
                }
            ))
        );
    }
}
