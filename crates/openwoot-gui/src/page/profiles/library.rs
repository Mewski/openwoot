use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use super::file::{EXTENSION, FileError, ProfileFile};
use crate::local_data;

const FOLDER: &str = "profiles";
const WORKER_STACK: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Stored {
    pub(super) path: PathBuf,
    pub(super) file: ProfileFile,
}

pub(super) fn directory() -> Result<PathBuf, FileError> {
    local_data::directory()
        .map(|data| data.join(FOLDER))
        .ok_or(FileError::NoDataDirectory)
}

pub(super) async fn on_worker<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, FileError> + Send + 'static,
) -> Result<T, FileError> {
    let (sender, receiver) = async_channel::bounded(1);
    thread::Builder::new()
        .name("profile files".to_owned())
        .stack_size(WORKER_STACK)
        .spawn(move || {
            sender.try_send(work()).ok();
        })
        .map_err(FileError::Worker)?;
    receiver
        .recv()
        .await
        .map_err(|_| FileError::WorkerStopped)?
}

pub(super) fn list(directory: &Path) -> Result<Vec<Stored>, FileError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(FileError::List {
                path: directory.to_owned(),
                source,
            });
        }
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == EXTENSION)
        })
        .collect();
    paths.sort();
    Ok(paths
        .into_iter()
        .filter_map(|path| {
            let file = ProfileFile::load(&path).ok()?;
            Some(Stored { path, file })
        })
        .collect())
}

pub(super) fn add(directory: &Path, file: ProfileFile) -> Result<Stored, FileError> {
    fs::create_dir_all(directory).map_err(|source| FileError::CreateDirectory {
        path: directory.to_owned(),
        source,
    })?;
    let path = free_path(directory, now_millis());
    file.save(&path)?;
    Ok(Stored { path, file })
}

pub(super) fn update(stored: &Stored) -> Result<(), FileError> {
    stored.file.save(&stored.path)
}

pub(super) fn remove(stored: &Stored) -> Result<(), FileError> {
    fs::remove_file(&stored.path).map_err(|source| FileError::Remove {
        path: stored.path.clone(),
        source,
    })
}

fn free_path(directory: &Path, start: u128) -> PathBuf {
    let mut stamp = start;
    loop {
        let path = directory.join(format!("{stamp}.{EXTENSION}"));
        if !path.exists() {
            return path;
        }
        stamp = stamp.saturating_add(1);
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn a_taken_stamp_moves_to_the_next_free_name() {
        let directory = env::temp_dir().join(format!("openwoot-library-{}", now_millis()));
        assert!(fs::create_dir_all(&directory).is_ok());
        assert!(fs::write(directory.join("7.json"), "{}").is_ok());
        assert_eq!(free_path(&directory, 7), directory.join("8.json"));
        assert_eq!(free_path(&directory, 9), directory.join("9.json"));
        assert!(fs::remove_dir_all(&directory).is_ok());
    }

    #[test]
    fn a_missing_library_lists_nothing() {
        let directory = env::temp_dir().join(format!("openwoot-missing-{}", now_millis()));
        assert_eq!(list(&directory).ok(), Some(Vec::new()));
    }
}
