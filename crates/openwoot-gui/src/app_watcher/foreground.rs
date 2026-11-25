use std::{
    path::{Path, PathBuf},
    process,
};

#[cfg(target_os = "macos")]
use objc2_app_kit::NSWorkspace;
use openwoot_protocol::apps::{self, App, Custom, Filter, Link, Matching};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Foreground {
    pub(crate) process: u32,
    pub(crate) name: String,
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunningApp {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
}

impl Foreground {
    #[cfg(target_os = "macos")]
    pub(crate) fn current(titled: bool) -> Option<Self> {
        let application = NSWorkspace::sharedWorkspace().frontmostApplication()?;
        let process = u32::try_from(application.processIdentifier()).ok()?;
        let paths = [application.executableURL(), application.bundleURL()]
            .into_iter()
            .flatten()
            .filter_map(|url| url.path())
            .map(|path| PathBuf::from(path.to_string()))
            .collect();
        Some(Self {
            process,
            name: application
                .localizedName()
                .map_or_else(String::new, |name| name.to_string()),
            paths,
            title: if titled {
                window_title(process)
            } else {
                String::new()
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn current(_titled: bool) -> Option<Self> {
        let window = x_win::get_active_window().ok()?;
        (window.info.process_id != 0).then(|| Self {
            process: window.info.process_id,
            name: window.info.name,
            paths: vec![PathBuf::from(window.info.path)],
            title: window.title,
        })
    }

    pub(crate) fn is_own(&self) -> bool {
        self.process == process::id()
    }
}

impl RunningApp {
    pub(crate) fn link(&self) -> Option<Link> {
        let filter = if cfg!(target_os = "macos") {
            Filter::ProcessName(text(self.path.file_name()?.to_string_lossy().as_ref())?)
        } else {
            Filter::ProcessDirectory(text(self.path.parent()?.to_string_lossy().as_ref())?)
        };
        Some(Link {
            name: text(&self.name)?,
            app: App::Custom(Custom {
                matching: Matching::All,
                filters: [filter].into_iter().collect(),
            }),
        })
    }
}

pub(crate) fn running_apps() -> Vec<RunningApp> {
    let own = process::id();
    let mut apps: Vec<RunningApp> = Vec::new();
    for window in x_win::get_open_windows().unwrap_or_default() {
        let path = PathBuf::from(&window.info.path);
        if window.info.process_id == own
            || window.info.path.is_empty()
            || apps.iter().any(|app| app.path == path)
        {
            continue;
        }
        let name = [window.info.name, window.title]
            .into_iter()
            .find(|name| !name.trim().is_empty())
            .or_else(|| stem(&path))
            .unwrap_or_default();
        apps.push(RunningApp { name, path });
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

pub(crate) fn text(value: &str) -> Option<apps::Text> {
    apps::Text::try_from(value.trim()).ok()
}

#[cfg(target_os = "macos")]
fn window_title(process: u32) -> String {
    x_win::get_open_windows()
        .unwrap_or_default()
        .into_iter()
        .find(|window| window.info.process_id == process)
        .map_or_else(String::new, |window| window.title)
}

fn stem(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}
