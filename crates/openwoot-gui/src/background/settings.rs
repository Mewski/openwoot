use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::local_data;

const FILE: &str = "background.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub(crate) running: bool,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SettingsError {
    #[error("there is no folder for openwoot's settings")]
    NoDataDirectory,
    #[error("could not encode the background settings")]
    Encode(#[from] serde_json::Error),
    #[error("could not write `{path}`")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl Settings {
    pub(crate) fn load() -> Self {
        path()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(self) -> Result<(), SettingsError> {
        let path = path().ok_or(SettingsError::NoDataDirectory)?;
        let bytes = serde_json::to_vec_pretty(&self)?;
        let written = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(&path, bytes));
        written.map_err(|source| SettingsError::Write { path, source })
    }
}

fn path() -> Option<PathBuf> {
    local_data::directory().map(|data| data.join(FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_leave_the_service_off() {
        let settings: Settings = serde_json::from_str("{}").unwrap_or(Settings { running: true });
        assert_eq!(settings, Settings { running: false });
    }
}
