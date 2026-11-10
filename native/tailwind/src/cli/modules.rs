//! The files of the JavaScript modules of `@plugin` and `@config`: the
//! file system of the computer, for reads only.

use super::{stamp, Stamp};
use crate::compat::module_loader::{FileKind, FileStat, ModuleLoader};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// The file system. It records each module file with its stamp from
/// before the read: a change of the file gives a full build in the watch.
#[derive(Default)]
pub struct FileSystem {
    dependencies: RefCell<Vec<(PathBuf, Stamp)>>,
}

impl FileSystem {
    pub fn take_dependencies(&self) -> Vec<(PathBuf, Stamp)> {
        std::mem::take(&mut *self.dependencies.borrow_mut())
    }
}

fn kind(file_type: std::fs::FileType) -> FileKind {
    if file_type.is_dir() {
        FileKind::Directory
    } else {
        FileKind::File
    }
}

impl ModuleLoader for FileSystem {
    fn read(&self, path: &str) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn stat(&self, path: &str) -> Option<FileStat> {
        let meta = std::fs::metadata(path).ok()?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0.0, |duration| duration.as_secs_f64() * 1000.0);
        Some(FileStat { kind: kind(meta.file_type()), size: meta.len(), modified_ms })
    }

    fn read_dir(&self, path: &str) -> Option<Vec<(String, FileKind)>> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(path).ok()?.flatten() {
            // A link has the kind of its target.
            let Ok(meta) = std::fs::metadata(entry.path()) else { continue };
            entries.push((entry.file_name().to_string_lossy().into_owned(), kind(meta.file_type())));
        }
        // The sequence of Node.js.
        entries.sort_by(|a, z| a.0.cmp(&z.0));
        Some(entries)
    }

    fn on_dependency(&self, path: &str) {
        let path = Path::new(path);
        self.dependencies.borrow_mut().push((path.to_path_buf(), stamp(path)));
    }

    fn console(&self, text: &str, is_error: bool) {
        if is_error {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }

    fn node_path(&self) -> Vec<String> {
        let Some(paths) = std::env::var_os("NODE_PATH") else { return Vec::new() };
        std::env::split_paths(&paths).map(|path| path.to_string_lossy().into_owned()).filter(|path| !path.is_empty()).collect()
    }

    fn cwd(&self) -> String {
        std::env::current_dir().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".to_string())
    }

    fn env(&self) -> Vec<(String, String)> {
        // Only `NODE_ENV`: a plugin does not get the environment of the user.
        std::env::var("NODE_ENV").ok().map(|value| ("NODE_ENV".to_string(), value)).into_iter().collect()
    }
}
