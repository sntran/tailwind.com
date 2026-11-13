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
    /// True: the messages of `console` are not printed.
    quiet: bool,
}

impl FileSystem {
    /// A file system whose `console` prints nothing when `quiet` is true.
    pub fn with_quiet_console(quiet: bool) -> Self {
        Self { quiet, ..Self::default() }
    }

    pub fn take_dependencies(&self) -> Vec<(PathBuf, Stamp)> {
        std::mem::take(&mut *self.dependencies.borrow_mut())
    }
}

/// The directories of the environment variable `NODE_PATH`.
///
/// The separator is `;` on Windows and `:` on the other systems, as in
/// Node.js. `tailwind.com` is one file for all the systems, so the system
/// is the system at run time. (`std::env::split_paths` is for the system
/// of the build, and is not there for WASI.)
pub fn node_path() -> Vec<PathBuf> {
    let Some(paths) = super::context::var("NODE_PATH") else { return Vec::new() };
    #[cfg(tw_js)]
    let windows = crate::compat::js::engine::platform() == "win32";
    // A build with no JavaScript engine has no C code for this question.
    #[cfg(not(tw_js))]
    let windows = cfg!(windows);
    let separator = if windows { ';' } else { ':' };
    paths.split(separator).filter(|path| !path.is_empty()).map(PathBuf::from).collect()
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

    fn read_bytes(&self, path: &str) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn is_symlink(&self, path: &str) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
    }

    fn real_path(&self, path: &str) -> Option<String> {
        let real = std::fs::canonicalize(path).ok()?;
        // Windows gives a path with the prefix `\\?\`: keep the path that
        // the module gave there.
        let text = real.to_string_lossy();
        Some(if text.starts_with('/') { text.into_owned() } else { path.to_string() })
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
        if self.quiet {
            return;
        }
        if is_error {
            super::context::err(&format!("{text}\n"));
        } else {
            super::context::out(&format!("{text}\n"));
        }
    }

    fn node_path(&self) -> Vec<String> {
        node_path().into_iter().map(|path| path.to_string_lossy().into_owned()).collect()
    }

    fn cwd(&self) -> String {
        let cwd = super::context::cwd();
        if cwd.as_os_str().is_empty() {
            "/".to_string()
        } else {
            cwd.to_string_lossy().into_owned()
        }
    }

    fn env(&self) -> Vec<(String, String)> {
        // The environment of the program, as `process.env` of Node.js. A
        // variable that is not Unicode text is not there.
        super::context::vars()
    }

    fn argv(&self) -> Vec<String> {
        // In a run of `run_with`, the arguments of the run, after the name
        // of the program.
        let mut args: Box<dyn Iterator<Item = String>> = match super::context::args() {
            Some(args) => Box::new(std::iter::once("tailwindcss".to_string()).chain(args)),
            None => Box::new(std::env::args_os().map(|arg| arg.to_string_lossy().into_owned())),
        };
        let program = args.next().unwrap_or_else(|| "tailwindcss".to_string());
        let mut argv = vec![program.clone(), program];
        argv.extend(args);
        argv
    }
}
