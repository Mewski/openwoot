use std::{env, path::PathBuf};

const APPLICATION: &str = "openwoot";

pub(crate) fn directory() -> Option<PathBuf> {
    let home = || {
        env::var_os("HOME")
            .filter(|home| !home.is_empty())
            .map(PathBuf::from)
    };
    let data = if cfg!(target_os = "macos") {
        home().map(|home| home.join("Library").join("Application Support"))
    } else if cfg!(windows) {
        env::var_os("APPDATA")
            .filter(|data| !data.is_empty())
            .map(PathBuf::from)
    } else {
        env::var_os("XDG_DATA_HOME")
            .filter(|data| !data.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join(".local").join("share")))
    };
    data.map(|data| data.join(APPLICATION))
}
