use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use openwoot_protocol::apps::{App, Link};
use steamlocate::SteamDir;

use super::foreground::text;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SteamApps {
    directories: HashMap<u32, PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledGame {
    pub(crate) id: u32,
    pub(crate) name: String,
}

impl SteamApps {
    pub(crate) fn locate(ids: impl IntoIterator<Item = u32>) -> Self {
        let Ok(steam) = SteamDir::locate() else {
            return Self::default();
        };
        Self::from_directories(ids.into_iter().filter_map(|id| {
            let (app, library) = steam.find_app(id).ok()??;
            Some((id, library.resolve_app_dir(&app)))
        }))
    }

    pub(crate) fn from_directories(directories: impl IntoIterator<Item = (u32, PathBuf)>) -> Self {
        Self {
            directories: directories.into_iter().collect(),
        }
    }

    pub(crate) fn directory(&self, id: u32) -> Option<&Path> {
        self.directories.get(&id).map(PathBuf::as_path)
    }
}

impl InstalledGame {
    pub(crate) fn link(&self) -> Option<Link> {
        Some(Link {
            name: text(&self.name)?,
            app: App::Steam(self.id),
        })
    }
}

pub(crate) fn installed_games() -> Vec<InstalledGame> {
    let Ok(steam) = SteamDir::locate() else {
        return Vec::new();
    };
    let Ok(libraries) = steam.libraries() else {
        return Vec::new();
    };
    let mut games: Vec<InstalledGame> = libraries
        .filter_map(Result::ok)
        .flat_map(|library| library.apps().filter_map(Result::ok).collect::<Vec<_>>())
        .map(|app| InstalledGame {
            id: app.app_id,
            name: app.name.unwrap_or(app.install_dir),
        })
        .collect();
    games.sort_by(|first, second| {
        first
            .name
            .to_lowercase()
            .cmp(&second.name.to_lowercase())
            .then(first.id.cmp(&second.id))
    });
    games.dedup_by_key(|game| game.id);
    games
}
